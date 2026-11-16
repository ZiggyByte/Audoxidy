//! Area tests for cover identity: owner-scoped file names and per-song overrides.
//!
//! Every test runs against an in-memory database and writes its files into a
//! private temporary directory, so no test touches the process-global cover cache
//! or the application database.

use super::*;
use std::path::Path;
use std::sync::Arc;

use crate::utils::covers::{effective_cover_path, process_and_save_cover_in};
use crate::utils::test_support::TempTestDir;

/// Encodes a solid PNG in memory so the writer reads a real image.
fn png_bytes(width: u32, height: u32) -> Vec<u8> {
    let image = image::RgbaImage::from_pixel(width, height, image::Rgba([200, 30, 30, 255]));
    let mut bytes = Vec::new();
    image::DynamicImage::ImageRgba8(image)
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .expect("no se pudo codificar el PNG de prueba");
    bytes
}

/// Inserts one song and returns the cover file name the insert reported.
fn insert_song(
    db: &mut Database,
    path: &str,
    album: &str,
    picture_hash: Option<&str>,
) -> (String, bool) {
    let song = SongData {
        full_file_path: Arc::from(path.to_string().into_boxed_str()),
        title: Some(Arc::from("Track".to_string().into_boxed_str())),
        artist: Some(Arc::from("The Artist".to_string().into_boxed_str())),
        album: Some(Arc::from(album.to_string().into_boxed_str())),
        ..Default::default()
    };

    db.insert_song_full(
        &song,
        &SongMetadataExtended::default(),
        picture_hash.map(str::to_owned),
        false,
    )
    .expect("la canción de prueba debe insertarse")
}

/// Identity hash of one album row.
fn album_hash_id(db: &Database, album: &str) -> String {
    db.get_all_albums()
        .expect("se deben leer los álbumes")
        .into_iter()
        .find(|row| row.1 == album)
        .map(|row| row.0)
        .expect("el álbum insertado debe existir")
}

/// Derived cover path of one album row.
fn album_cover_path(db: &Database, album: &str) -> Option<String> {
    db.get_all_albums()
        .expect("se deben leer los álbumes")
        .into_iter()
        .find(|row| row.1 == album)
        .and_then(|row| row.5)
}

/// Derived cover path of one album row, reduced to its file name.
fn album_cover_file_name(db: &Database, album: &str) -> String {
    album_cover_path(db, album)
        .as_deref()
        .and_then(|path| Path::new(path).file_name())
        .and_then(|name| name.to_str())
        .map(str::to_owned)
        .expect("el álbum debe tener una carátula almacenada")
}

/// Per-song overridden cover hash of one song row, when it has one.
fn song_cover_override(db: &Database, song_id: i64) -> Option<String> {
    db.cover_refs_for_songs(&[song_id])
        .expect("se deben leer las referencias de carátula")
        .into_iter()
        .next()
        .and_then(|(_, _, override_hash, _)| override_hash)
}

#[test]
fn test_identical_images_produce_two_album_files() {
    let dir = TempTestDir::new("db-cover");
    let mut db = Database::new_memory().expect("se debe crear la base en memoria");
    let shared_picture = "ab12cd34";

    let (alpha_name, alpha_processing) = insert_song(
        &mut db,
        "/music/alpha/track.flac",
        "Album Alpha",
        Some(shared_picture),
    );
    let (beta_name, beta_processing) = insert_song(
        &mut db,
        "/music/beta/track.flac",
        "Album Beta",
        Some(shared_picture),
    );

    assert!(
        alpha_processing && beta_processing,
        "cada álbum escribe su propia carátula"
    );
    assert_ne!(
        alpha_name, beta_name,
        "dos álbumes con la misma imagen no pueden compartir archivo"
    );
    assert_eq!(
        alpha_name,
        format!(
            "{}-{shared_picture}.avif",
            album_hash_id(&db, "Album Alpha")
        ),
        "el nombre de álbum lleva su propio hash y el del contenido"
    );
    assert_eq!(
        beta_name,
        format!("{}-{shared_picture}.avif", album_hash_id(&db, "Album Beta"))
    );
    assert_eq!(album_cover_file_name(&db, "Album Alpha"), alpha_name);
    assert_eq!(album_cover_file_name(&db, "Album Beta"), beta_name);

    let picture = png_bytes(4, 4);
    for file_name in [&alpha_name, &beta_name] {
        let written = process_and_save_cover_in(dir.path(), &picture, file_name)
            .expect("la carátula de prueba debe escribirse");
        assert_eq!(
            written.file_name().and_then(|name| name.to_str()),
            Some(file_name.as_str()),
            "el nombre escrito es exactamente el nombre almacenado"
        );
        assert!(
            dir.path().join(file_name).exists(),
            "cada nombre produce su propio archivo"
        );
    }
}

#[test]
fn test_written_cover_name_matches_the_stored_path() {
    let dir = TempTestDir::new("db-cover");
    let mut db = Database::new_memory().expect("se debe crear la base en memoria");

    let (file_name, needs_processing) = insert_song(
        &mut db,
        "/music/solo/track.flac",
        "Solo Album",
        Some("00ff00ff"),
    );
    assert!(needs_processing, "la primera carátula del álbum se escribe");

    let stored = album_cover_file_name(&db, "Solo Album");
    assert_eq!(
        stored, file_name,
        "la ruta derivada y el nombre devuelto al escáner son el mismo valor"
    );
    assert_eq!(
        stored.matches(".avif").count(),
        1,
        "la extensión aparece una sola vez: una capa que la añadiera otra vez dejaría un archivo ausente"
    );

    let written = process_and_save_cover_in(dir.path(), &png_bytes(4, 4), &stored)
        .expect("la carátula de prueba debe escribirse");
    assert_eq!(
        written.file_name().and_then(|name| name.to_str()),
        Some(stored.as_str())
    );
    assert!(
        dir.path().join(&stored).exists(),
        "unir el nombre devuelto encuentra el archivo escrito: una extensión duplicada dejaría un archivo ausente"
    );
}

#[test]
fn test_override_only_when_image_differs() {
    let mut db = Database::new_memory().expect("se debe crear la base en memoria");
    let album_picture = "1111aaaa";
    let other_picture = "2222bbbb";

    let (_, processing) = insert_song(
        &mut db,
        "/music/merged/one.flac",
        "Merged Album",
        Some(album_picture),
    );
    assert!(processing, "la carátula del álbum se escribe una vez");

    let (_, same_processing) = insert_song(
        &mut db,
        "/music/merged/two.flac",
        "Merged Album",
        Some(album_picture),
    );
    assert!(
        !same_processing,
        "la misma imagen que la del álbum no produce archivo propio"
    );

    let (_, different_processing) = insert_song(
        &mut db,
        "/music/merged/three.flac",
        "Merged Album",
        Some(other_picture),
    );
    assert!(
        different_processing,
        "una imagen distinta del álbum sí produce carátula propia"
    );

    let songs = db.get_all_songs().expect("se deben leer las canciones");

    let inherited = songs
        .iter()
        .find(|song| song.full_file_path.as_ref() == "/music/merged/two.flac")
        .expect("la segunda canción debe existir");
    assert!(
        song_cover_override(&db, inherited.id).is_none(),
        "heredar la carátula del álbum no deja override"
    );

    let overridden = songs
        .iter()
        .find(|song| song.full_file_path.as_ref() == "/music/merged/three.flac")
        .expect("la tercera canción debe existir");
    let stored_hash = song_cover_override(&db, overridden.id)
        .expect("la carátula distinta se guarda como override");
    assert_eq!(
        stored_hash,
        Database::generate_hash(&format!("{}{}", "/music/merged/three.flac", other_picture)),
        "el override guarda el hash puro del contenido"
    );

    let album_hash = album_hash_id(&db, "Merged Album");
    let effective = effective_cover_path(
        Some(&album_hash),
        Some(album_picture),
        Some(stored_hash.as_str()),
        overridden.id,
    )
    .expect("un override siempre resuelve una ruta");
    let effective_file_name = Path::new(&effective)
        .file_name()
        .and_then(|name| name.to_str())
        .expect("la ruta efectiva debe tener un nombre de archivo");
    assert_eq!(
        effective_file_name,
        format!("song-{}-{stored_hash}.avif", overridden.id),
        "el archivo propio se nombra por el id de la canción y el hash del contenido"
    );
}

#[test]
fn test_album_cover_file_name_embeds_album_and_content_hash() {
    let album_hash = "aa11bb22";
    let cover_hash = "cc33dd44";

    let file_name = crate::utils::covers::album_cover_file_name(album_hash, cover_hash)
        .expect("dos hashes hexadecimales componen un nombre");
    assert_eq!(file_name, format!("{album_hash}-{cover_hash}.avif"));
    assert_eq!(
        file_name.matches(".avif").count(),
        1,
        "el nombre lleva la extensión una sola vez"
    );
    assert_eq!(
        crate::utils::covers::album_cover_stem(album_hash, cover_hash),
        Ok(format!("{album_hash}-{cover_hash}")),
        "el tallo es el nombre sin extensión"
    );

    // The same picture under a different owner composes a different file.
    let other = crate::utils::covers::album_cover_file_name("ee55ff66", cover_hash)
        .expect("el otro álbum también compone un nombre");
    assert_ne!(file_name, other);

    // The write path stores exactly this composition for a real album row.
    let mut db = Database::new_memory().expect("se debe crear la base en memoria");
    let (stored_name, needs_processing) = insert_song(
        &mut db,
        "/music/embed/track.flac",
        "Embed Album",
        Some(cover_hash),
    );
    assert!(needs_processing);
    assert_eq!(
        stored_name,
        format!("{}-{cover_hash}.avif", album_hash_id(&db, "Embed Album")),
        "el nombre escrito lleva el hash del álbum y el del contenido"
    );
}

#[test]
fn test_song_cover_file_name_embeds_owner_and_content_hash() {
    let cover_hash = "cc33dd44";

    let file_name = crate::utils::covers::song_cover_file_name(42, cover_hash)
        .expect("un hash hexadecimal compone un nombre propio");
    assert_eq!(file_name, format!("song-42-{cover_hash}.avif"));
    assert_eq!(
        crate::utils::covers::song_cover_stem(42, cover_hash),
        Ok(format!("song-42-{cover_hash}")),
        "el tallo es el nombre sin extensión"
    );

    // A different owner never shares the file.
    let other = crate::utils::covers::song_cover_file_name(43, cover_hash)
        .expect("otra canción también compone un nombre");
    assert_ne!(file_name, other);

    // The resolved path the read layer hands out ends in this same file name.
    let resolved = effective_cover_path(Some("aa11bb22"), Some("cc33dd44"), Some(cover_hash), 42)
        .expect("un override siempre resuelve una ruta");
    assert_eq!(
        Path::new(&resolved)
            .file_name()
            .and_then(|name| name.to_str()),
        Some(file_name.as_str())
    );
}

#[test]
fn test_referenced_cover_file_names_includes_album_and_override_files() {
    let mut db = Database::new_memory().expect("se debe crear la base en memoria");

    // The first song gives the album its cover; the second brings different art,
    // so it owns a file of its own.
    insert_song(
        &mut db,
        "/music/refs/one.flac",
        "Refs Album",
        Some("aaaa1111"),
    );
    insert_song(
        &mut db,
        "/music/refs/two.flac",
        "Refs Album",
        Some("bbbb2222"),
    );

    let album_name = album_cover_file_name(&db, "Refs Album");
    let overridden_id = db
        .get_song_id_by_path("/music/refs/two.flac")
        .expect("se debe leer el id de la canción")
        .expect("la segunda canción debe existir");
    let override_hash = song_cover_override(&db, overridden_id)
        .expect("la imagen distinta del álbum produce un override");
    let override_name = format!("song-{overridden_id}-{override_hash}.avif");

    let referenced = db
        .referenced_cover_file_names()
        .expect("el conjunto de referencias debe leerse");

    assert_eq!(
        referenced.len(),
        2,
        "una carátula de álbum y una carátula propia, sin entradas inventadas"
    );
    assert!(
        referenced.contains(std::ffi::OsStr::new(&album_name)),
        "la ruta derivada del álbum aporta su nombre de archivo"
    );
    assert!(
        referenced.contains(std::ffi::OsStr::new(&override_name)),
        "el override aporta el nombre que compone con su id de canción"
    );
    for entry in &referenced {
        assert_eq!(
            Path::new(entry).components().count(),
            1,
            "el conjunto guarda nombres de archivo, nunca rutas completas"
        );
    }
}

#[test]
fn test_referenced_cover_file_names_skips_null_overrides() {
    let mut db = Database::new_memory().expect("se debe crear la base en memoria");

    insert_song(
        &mut db,
        "/music/refs/one.flac",
        "Inherit Album",
        Some("cccc3333"),
    );
    // No embedded picture: the song inherits the album cover and must not add a
    // name of its own.
    insert_song(&mut db, "/music/refs/two.flac", "Inherit Album", None);

    let album_name = album_cover_file_name(&db, "Inherit Album");
    let referenced = db
        .referenced_cover_file_names()
        .expect("el conjunto de referencias debe leerse");

    assert_eq!(
        referenced.len(),
        1,
        "solo la carátula del álbum está referenciada"
    );
    assert!(referenced.contains(std::ffi::OsStr::new(&album_name)));
    assert!(
        !referenced
            .iter()
            .any(|name| name.to_string_lossy().starts_with("song-")),
        "una canción sin override no inventa un nombre propio"
    );
}

#[test]
fn test_album_read_paths_derive_the_cover_path() {
    let mut db = Database::new_memory().expect("se debe crear la base en memoria");

    insert_song(
        &mut db,
        "/music/derive/alpha.flac",
        "Derive Alpha",
        Some("aaaa1111"),
    );
    insert_song(
        &mut db,
        "/music/derive/beta.flac",
        "Derive Beta",
        Some("bbbb2222"),
    );

    let albums = db.get_all_albums().expect("se deben leer los álbumes");
    let grid = db
        .get_grid_items_by_artist()
        .expect("se debe leer la rejilla de álbumes");
    assert_eq!(albums.len(), 2);
    assert_eq!(grid.len(), 2);

    for (album, cover_hash) in [("Derive Alpha", "aaaa1111"), ("Derive Beta", "bbbb2222")] {
        let album_hash = album_hash_id(&db, album);
        let expected = crate::utils::covers::album_cover_file_name(&album_hash, cover_hash)
            .expect("los dos hashes del álbum componen su nombre");

        for rows in [&albums, &grid] {
            let row = rows
                .iter()
                .find(|row| row.1 == album)
                .expect("el álbum debe aparecer en la lectura");
            let path = row.5.as_deref().expect("el álbum debe derivar una ruta");
            assert_eq!(
                Path::new(path).file_name().and_then(|name| name.to_str()),
                Some(expected.as_str()),
                "la ruta derivada de {album} lleva el nombre compuesto de sus hashes"
            );
            assert_eq!(row.0, album_hash, "la fila lleva el hash del álbum");
        }
    }
}

#[test]
fn test_song_rows_resolve_the_override_over_the_album_cover() {
    let mut db = Database::new_memory().expect("se debe crear la base en memoria");

    insert_song(
        &mut db,
        "/music/resolve/base.flac",
        "Resolve Album",
        Some("aaaa1111"),
    );
    insert_song(
        &mut db,
        "/music/resolve/alt.flac",
        "Resolve Album",
        Some("bbbb2222"),
    );

    let album_hash = album_hash_id(&db, "Resolve Album");
    let album_name = crate::utils::covers::album_cover_file_name(&album_hash, "aaaa1111")
        .expect("los hashes del álbum componen su nombre");
    let base_id = db
        .get_song_id_by_path("/music/resolve/base.flac")
        .expect("se debe leer el id de la canción base")
        .expect("la canción base existe");
    let override_id = db
        .get_song_id_by_path("/music/resolve/alt.flac")
        .expect("se debe leer el id de la canción con override")
        .expect("la canción con override existe");
    let override_hash = song_cover_override(&db, override_id)
        .expect("la imagen distinta del álbum produce un override");
    let override_name = crate::utils::covers::song_cover_file_name(override_id, &override_hash)
        .expect("el hash del override compone un nombre");

    let playlist_id = db
        .create_playlist("Resolve Playlist", false)
        .expect("se debe crear la playlist");
    db.add_song_to_playlist(playlist_id, base_id)
        .expect("la canción base entra en la playlist");
    db.add_song_to_playlist(playlist_id, override_id)
        .expect("la canción con override entra en la playlist");

    let file_name_of = |path: Option<&str>| -> String {
        path.and_then(|path| Path::new(path).file_name())
            .and_then(|name| name.to_str())
            .map(str::to_owned)
            .expect("la ruta resuelta debe tener un nombre de archivo")
    };

    let all_songs = db.get_all_songs().expect("se deben leer las canciones");
    let filtered = db
        .get_library_songs_filtered(&LibrarySearchParams::default())
        .expect("se debe leer la biblioteca filtrada");
    let by_album = db
        .get_songs_by_album(&album_hash)
        .expect("se deben leer las canciones del álbum");
    let playlist = db
        .get_playlist_songs(playlist_id)
        .expect("se deben leer las canciones de la playlist");

    for songs in [&all_songs, &filtered, &by_album] {
        let base = songs
            .iter()
            .find(|song| song.id == base_id)
            .expect("la canción base aparece en cada lectura");
        assert_eq!(
            file_name_of(base.compressed_cached_cover_root.as_deref()),
            album_name,
            "sin override la fila resuelve la carátula del álbum"
        );

        let overridden = songs
            .iter()
            .find(|song| song.id == override_id)
            .expect("la canción con override aparece en cada lectura");
        assert_eq!(
            file_name_of(overridden.compressed_cached_cover_root.as_deref()),
            override_name,
            "el override gana sobre la carátula del álbum"
        );
    }

    for song in &playlist {
        let expected = if song.song_id == override_id {
            &override_name
        } else {
            assert_eq!(song.song_id, base_id);
            &album_name
        };
        assert_eq!(
            file_name_of(song.cover_path.as_deref()),
            expected.as_str(),
            "la lectura de playlist resuelve el mismo nombre que la biblioteca"
        );
    }
}

#[test]
fn test_sweep_set_contains_composed_album_names() {
    let mut db = Database::new_memory().expect("se debe crear la base en memoria");

    insert_song(
        &mut db,
        "/music/sweepset/one.flac",
        "Sweep Set Album",
        Some("aaaa1111"),
    );
    insert_song(
        &mut db,
        "/music/sweepset/two.flac",
        "Sweep Set Album",
        Some("bbbb2222"),
    );

    let album_hash = album_hash_id(&db, "Sweep Set Album");
    let album_name = crate::utils::covers::album_cover_file_name(&album_hash, "aaaa1111")
        .expect("los hashes del álbum componen su nombre");
    let override_id = db
        .get_song_id_by_path("/music/sweepset/two.flac")
        .expect("se debe leer el id de la canción")
        .expect("la canción con override existe");
    let override_hash = song_cover_override(&db, override_id)
        .expect("la imagen distinta del álbum produce un override");
    let override_name = crate::utils::covers::song_cover_file_name(override_id, &override_hash)
        .expect("el hash del override compone un nombre");

    let expected: std::collections::HashSet<std::ffi::OsString> = [album_name, override_name]
        .into_iter()
        .map(std::ffi::OsString::from)
        .collect();

    let referenced = db
        .referenced_cover_file_names()
        .expect("el conjunto de referencias debe leerse");
    assert_eq!(
        referenced, expected,
        "el conjunto es exactamente los nombres compuestos de los hashes"
    );
    for entry in &referenced {
        assert_eq!(
            Path::new(entry).components().count(),
            1,
            "el conjunto guarda nombres de archivo, nunca rutas con separadores"
        );
    }
}

#[test]
fn test_targeted_delete_checks_the_hash_not_a_path() {
    let mut db = Database::new_memory().expect("se debe crear la base en memoria");

    insert_song(
        &mut db,
        "/music/targeted/remove.flac",
        "Targeted Album",
        Some("aaaa1111"),
    );
    insert_song(
        &mut db,
        "/music/targeted/keep.flac",
        "Kept Album",
        Some("bbbb2222"),
    );

    let removed_hash = album_hash_id(&db, "Targeted Album");
    let kept_hash = album_hash_id(&db, "Kept Album");

    assert!(
        db.is_cover_hash_referenced(&kept_hash, "bbbb2222")
            .expect("la comprobación por hash debe responder"),
        "el álbum que sobrevive sigue referenciando su carátula"
    );
    assert!(
        !db.is_cover_hash_referenced(&removed_hash, "bbbb2222")
            .expect("la comprobación por hash debe responder"),
        "un hash de carátula ajeno no está referenciado por el álbum"
    );

    db.delete_album_group(&removed_hash)
        .expect("el álbum debe borrarse");

    assert!(
        !db.is_cover_hash_referenced(&removed_hash, "aaaa1111")
            .expect("la comprobación por hash debe responder"),
        "el álbum borrado ya no referencia su carátula, sin consultar ninguna ruta"
    );
}
