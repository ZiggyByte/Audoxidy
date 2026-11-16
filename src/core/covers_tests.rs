//! Tests for the cover service: name composition, resolution and the orphan sweep.
//!
//! Every test builds its `Core` over a database and a cover cache inside a
//! private `TempTestDir`, so the directory is injected instead of read from the
//! process paths. No test opens the real cover cache or starts the cover worker,
//! and the binding that holds the guard lives for the whole body, so the
//! directory is removed when the test ends — a panicking test included.

use super::*;
use crate::core::Core;
use crate::core::types::CorePaths;
use crate::db::Database;
use crate::db::database::{SongData, SongMetadataExtended};
use crate::db::scanner::Scanner;
use crate::utils::test_support::TempTestDir;
use std::sync::{Arc, Mutex};

/// Builds a Core whose database and cover cache live in a private temporary
/// directory, so a delete or a sweep can be observed on disk without touching
/// the process-global cover cache.
fn make_core_with_temp_cover_dir() -> (Core, TempTestDir) {
    let dir = TempTestDir::new("core-covers");
    let cover_cache_dir = dir.path().join("covers");
    std::fs::create_dir_all(&cover_cache_dir)
        .expect("no se pudo crear el directorio temporal de carátulas");

    let paths = CorePaths {
        db_path: dir.path().join("library.db"),
        cover_cache_dir,
        config_path: dir.path().join("config.toml"),
    };
    let db = Arc::new(Mutex::new(
        Database::new_at(&paths.db_path).expect("no se pudo abrir la base de datos temporal"),
    ));
    let scanner = Arc::new(Scanner::new(db.clone()));
    let audio_manager = Arc::new(crate::audio::AudioManager::new().expect("audio manager init"));

    (Core::new(audio_manager, db, scanner, paths), dir)
}

/// Inserts one non-external song and returns its row id.
fn insert_song(core: &Core, path: &str, album: &str, picture_hash: Option<&str>) -> i64 {
    let song = SongData {
        full_file_path: Arc::from(path.to_string().into_boxed_str()),
        title: Some(Arc::from("Track".to_string().into_boxed_str())),
        artist: Some(Arc::from("The Artist".to_string().into_boxed_str())),
        album: Some(Arc::from(album.to_string().into_boxed_str())),
        ..Default::default()
    };

    let mut db = core.database().lock().expect("database lock");
    db.insert_song_full(
        &song,
        &SongMetadataExtended::default(),
        picture_hash.map(str::to_owned),
        false,
    )
    .expect("no se pudo insertar la canción de prueba");

    db.get_song_id_by_path(path)
        .expect("no se pudo leer el id de la canción")
        .expect("la canción insertada debe existir")
}

/// Derived cover path of the album, or `None` when the row is gone.
fn album_cover_path(core: &Core, album: &str) -> Option<String> {
    let db = core.database().lock().expect("database lock");
    db.get_all_albums()
        .expect("no se pudieron leer los álbumes")
        .into_iter()
        .find(|row| row.1 == album)
        .and_then(|row| row.5)
}

/// File name of the album's derived cover path.
fn album_cover_file_name(core: &Core, album: &str) -> String {
    album_cover_path(core, album)
        .as_deref()
        .and_then(|path| std::path::Path::new(path).file_name())
        .and_then(|name| name.to_str())
        .map(str::to_owned)
        .expect("el álbum debe tener una carátula almacenada")
}

/// Per-song cover override hash stored for a song, when it has one.
fn song_cover_override(core: &Core, song_id: i64) -> Option<String> {
    let db = core.database().lock().expect("database lock");
    db.cover_refs_for_songs(&[song_id])
        .expect("se deben leer las referencias de carátula")
        .into_iter()
        .next()
        .and_then(|(_, _, override_hash, _)| override_hash)
}

/// Writes a stand-in cover file into the injected cache directory.
fn write_cover(core: &Core, file_name: &str) -> PathBuf {
    let path = core.paths().cover_cache_dir.join(file_name);
    std::fs::write(&path, b"cover bytes").expect("no se pudo escribir la carátula de prueba");
    path
}

#[test]
fn test_resolve_cover_path_missing() {
    let (core, _dir) = make_core_with_temp_cover_dir();
    // Both hashes are valid hex, so the name composes and only the missing
    // file can make the resolution fail.
    let result = core.resolve_cover_path("a1b2c3", "0f0f0f");
    assert!(result.is_none());
}

#[test]
fn test_resolve_cover_path_existing() {
    let (core, _dir) = make_core_with_temp_cover_dir();
    let path = write_cover(&core, "a1b2c3-d4e5f6.avif");

    let resolved = core.resolve_cover_path("a1b2c3", "d4e5f6");
    assert_eq!(resolved, Some(path));
}

#[test]
fn test_purge_old_covers_does_not_panic() {
    let (core, _dir) = make_core_with_temp_cover_dir();
    write_cover(&core, "1111aaaa-2222bbbb.avif");
    write_cover(&core, "3333cccc-4444dddd.avif");

    let first = core
        .sweep_orphan_covers(false, false)
        .expect("el barrido debe completarse");
    assert_eq!(first.removed, 2, "las dos carátulas sin referencias se van");
    assert_eq!(first.failed, 0);
    assert_eq!(first.outcome, SweepOutcome::Completed);

    let second = core
        .sweep_orphan_covers(false, false)
        .expect("un segundo barrido también debe completarse");
    assert_eq!(second.removed, 0, "no queda nada por eliminar");
}

#[test]
fn test_clear_cover_cache_does_not_panic() {
    let (core, _dir) = make_core_with_temp_cover_dir();
    core.clear_cover_cache();
}

#[test]
fn test_clear_cover_lru_cache_does_not_panic() {
    let (core, _dir) = make_core_with_temp_cover_dir();

    let report = core
        .sweep_orphan_covers(false, false)
        .expect("un directorio vacío no es un error");
    assert_eq!(report.removed, 0);
    assert_eq!(report.failed, 0);
    assert_eq!(report.outcome, SweepOutcome::Completed);

    core.clear_cover_cache();
}

#[test]
fn test_sweep_removes_unreferenced_avif_files() {
    let (core, _dir) = make_core_with_temp_cover_dir();
    insert_song(
        &core,
        "/music/sweep/track.flac",
        "Sweep Album",
        Some("ab12cd34"),
    );
    let referenced_name = album_cover_file_name(&core, "Sweep Album");
    let referenced_file = write_cover(&core, &referenced_name);
    write_cover(&core, "9999aaaa-8888bbbb.avif");
    write_cover(&core, "7777cccc-6666dddd.avif");
    std::fs::write(core.paths().cover_cache_dir.join("notes.txt"), b"keep")
        .expect("no se pudo escribir el archivo ajeno");
    let nested = core.paths().cover_cache_dir.join("nested");
    std::fs::create_dir_all(&nested).expect("no se pudo crear el subdirectorio");

    let report = core
        .sweep_orphan_covers(false, false)
        .expect("el barrido debe completarse");
    assert_eq!(report.removed, 2, "solo las dos carátulas sin dueño se van");
    assert_eq!(report.failed, 0);
    assert_eq!(report.outcome, SweepOutcome::Completed);
    assert!(
        referenced_file.exists(),
        "la carátula referenciada se queda"
    );
    assert!(
        core.paths().cover_cache_dir.join("notes.txt").exists(),
        "un archivo que no es avif no se toca"
    );
    assert!(nested.exists(), "un subdirectorio no se toca");
}

#[test]
fn test_sweep_ignores_non_avif_entries() {
    let (core, _dir) = make_core_with_temp_cover_dir();
    let notes = core.paths().cover_cache_dir.join("notes.txt");
    std::fs::write(&notes, b"keep").expect("no se pudo escribir el archivo ajeno");
    let nested = core.paths().cover_cache_dir.join("nested");
    std::fs::create_dir_all(&nested).expect("no se pudo crear el subdirectorio");
    let nested_cover = nested.join("inside.avif");
    std::fs::write(&nested_cover, b"keep").expect("no se pudo escribir la carátula anidada");

    let report = core
        .sweep_orphan_covers(false, false)
        .expect("el barrido debe completarse");
    assert_eq!(report.removed, 0, "ninguna entrada elegible es un huérfano");
    assert!(notes.exists(), "el archivo de texto sigue ahí");
    assert!(nested_cover.exists(), "no se recorre el subdirectorio");
}

#[test]
fn test_sweep_skipped_while_scanning() {
    let (core, _dir) = make_core_with_temp_cover_dir();
    let orphan = write_cover(&core, "aaaa1111-bbbb2222.avif");

    let report = core
        .sweep_orphan_covers(true, false)
        .expect("un barrido omitido no es un error");
    assert_eq!(report.outcome, SweepOutcome::SkippedScanning);
    assert_eq!(report.removed, 0);
    assert!(orphan.exists(), "un escaneo activo impide borrar nada");
}

#[test]
fn test_sweep_skipped_while_cover_work_in_flight() {
    let (core, _dir) = make_core_with_temp_cover_dir();
    let orphan = write_cover(&core, "cccc3333-dddd4444.avif");

    let report = core
        .sweep_orphan_covers(false, true)
        .expect("un barrido omitido no es un error");
    assert_eq!(report.outcome, SweepOutcome::SkippedCoverWork);
    assert_eq!(report.removed, 0);
    assert!(
        orphan.exists(),
        "una escritura en curso puede ser el dueño de ese archivo"
    );
}

#[test]
fn test_sweep_runs_while_decodes_are_active() {
    let _guard = crate::utils::covers::GLOBAL_STATE_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let (core, _dir) = make_core_with_temp_cover_dir();
    let orphan = write_cover(&core, "eeee5555-ffff6666.avif");

    // A decode only reads a cover file, so a running decode cannot make an
    // unreferenced file exist. The gauge is raised here to prove the sweep is
    // decided by the scan and the writes, not by the decode count.
    crate::utils::covers::cover_metrics().increment_in_flight();
    let report = core
        .sweep_orphan_covers(false, false)
        .expect("una decodificación en curso no impide el barrido");
    crate::utils::covers::cover_metrics().decrement_in_flight();

    assert_eq!(report.outcome, SweepOutcome::Completed);
    assert_eq!(report.removed, 1, "el huérfano se elimina");
    assert!(!orphan.exists(), "la carátula sin dueño desaparece");
}

#[test]
fn test_sweep_missing_directory_is_not_an_error() {
    let (core, _dir) = make_core_with_temp_cover_dir();
    std::fs::remove_dir_all(core.paths().cover_cache_dir.clone())
        .expect("no se pudo quitar el directorio de carátulas");

    let report = core
        .sweep_orphan_covers(false, false)
        .expect("un directorio ausente no es un error");
    assert_eq!(report.removed, 0);
    assert_eq!(report.failed, 0);
    assert_eq!(report.outcome, SweepOutcome::Completed);
}

#[test]
fn test_sweep_after_delete_removes_only_the_orphan() {
    let (core, _dir) = make_core_with_temp_cover_dir();
    let shared_picture = "ab12cd34";

    let alpha_song = insert_song(
        &core,
        "/music/alpha/track.flac",
        "Album Alpha",
        Some(shared_picture),
    );
    insert_song(
        &core,
        "/music/beta/track.flac",
        "Album Beta",
        Some(shared_picture),
    );

    let alpha_name = album_cover_file_name(&core, "Album Alpha");
    let beta_name = album_cover_file_name(&core, "Album Beta");
    assert_ne!(alpha_name, beta_name, "cada álbum tiene su propio archivo");

    let alpha_file = write_cover(&core, &alpha_name);
    let beta_file = write_cover(&core, &beta_name);
    let extra_orphan = write_cover(&core, "5555eeee-6666ffff.avif");

    core.delete_songs_with_transaction(&[alpha_song])
        .expect("el borrado del álbum debe completarse");

    let report = core
        .sweep_orphan_covers(false, false)
        .expect("el barrido debe completarse");
    assert_eq!(
        report.removed, 1,
        "el borrado dirigido ya quitó la carátula del álbum borrado"
    );
    assert_eq!(report.failed, 0);
    assert!(
        !alpha_file.exists(),
        "la carátula del álbum borrado no vuelve"
    );
    assert!(!extra_orphan.exists(), "el huérfano sin dueño se va");
    assert!(
        beta_file.exists(),
        "la carátula del otro álbum no puede tocarse, aunque la imagen sea idéntica"
    );
    assert_eq!(
        album_cover_file_name(&core, "Album Beta"),
        beta_name,
        "la ruta derivada del álbum que sobrevive no cambia"
    );
    assert!(
        album_cover_path(&core, "Album Alpha").is_none(),
        "el álbum sin canciones no debe sobrevivir a la limpieza"
    );
}

#[test]
fn test_delete_album_removes_only_its_own_cover() {
    let (core, _dir) = make_core_with_temp_cover_dir();
    let shared_picture = "ab12cd34";

    let alpha_song = insert_song(
        &core,
        "/music/alpha/track.flac",
        "Album Alpha",
        Some(shared_picture),
    );
    insert_song(
        &core,
        "/music/beta/track.flac",
        "Album Beta",
        Some(shared_picture),
    );

    let alpha_name = album_cover_file_name(&core, "Album Alpha");
    let beta_name = album_cover_file_name(&core, "Album Beta");
    assert_ne!(
        alpha_name, beta_name,
        "two albums must never share a cover file name"
    );

    let alpha_file = write_cover(&core, &alpha_name);
    let beta_file = write_cover(&core, &beta_name);

    core.delete_songs_with_transaction(&[alpha_song])
        .expect("el borrado del álbum debe completarse");

    assert!(
        !alpha_file.exists(),
        "la carátula del álbum borrado debe desaparecer"
    );
    assert!(
        beta_file.exists(),
        "la carátula de otro álbum no puede tocarse, aunque la imagen sea idéntica"
    );
    assert_eq!(
        album_cover_file_name(&core, "Album Beta"),
        beta_name,
        "la ruta derivada del álbum que sobrevive no cambia"
    );
    assert!(
        album_cover_path(&core, "Album Alpha").is_none(),
        "el álbum sin canciones no debe sobrevivir a la limpieza"
    );
}

#[test]
fn test_delete_song_with_cover_override_removes_only_its_file() {
    let (core, _dir) = make_core_with_temp_cover_dir();

    let album_song = insert_song(
        &core,
        "/music/override/base.flac",
        "Album Override",
        Some("aaaa1111"),
    );
    let override_song = insert_song(
        &core,
        "/music/override/alt.flac",
        "Album Override",
        Some("bbbb2222"),
    );

    let album_file = write_cover(&core, &album_cover_file_name(&core, "Album Override"));
    let override_hash =
        song_cover_override(&core, override_song).expect("la canción debe tener un override");
    let override_name = crate::utils::covers::song_cover_file_name(override_song, &override_hash)
        .expect("el hash del override debe ser válido");
    let override_file = write_cover(&core, &override_name);
    assert_ne!(
        album_file, override_file,
        "la canción con arte propio no comparte el archivo del álbum"
    );

    core.delete_songs_with_transaction(&[override_song])
        .expect("el borrado de la canción debe completarse");

    assert!(
        !override_file.exists(),
        "la carátula propia de la canción borrada debe desaparecer"
    );
    assert!(
        album_file.exists(),
        "la carátula del álbum sigue referenciada por la otra canción"
    );
    assert!(
        song_cover_override(&core, album_song).is_none(),
        "la canción base conserva la carátula del álbum"
    );
}

#[test]
fn test_delete_keeps_cover_still_referenced() {
    let (core, _dir) = make_core_with_temp_cover_dir();

    let first_song = insert_song(
        &core,
        "/music/shared/one.flac",
        "Shared Album",
        Some("00ff00ff"),
    );
    insert_song(&core, "/music/shared/two.flac", "Shared Album", None);

    let cover_name = album_cover_file_name(&core, "Shared Album");
    let cover_file = write_cover(&core, &cover_name);
    let stored_path = album_cover_path(&core, "Shared Album");

    core.delete_songs_with_transaction(&[first_song])
        .expect("el borrado de una canción debe completarse");

    assert!(
        cover_file.exists(),
        "la carátula sigue referenciada por la canción restante"
    );
    assert_eq!(
        album_cover_path(&core, "Shared Album"),
        stored_path,
        "la ruta derivada del álbum que sobrevive no cambia"
    );
}

#[test]
fn test_delete_reads_cover_paths_before_commit() {
    let (core, _dir) = make_core_with_temp_cover_dir();

    let song = insert_song(
        &core,
        "/music/solo/track.flac",
        "Solo Album",
        Some("0badf00d"),
    );
    let cover_name = album_cover_file_name(&core, "Solo Album");
    let cover_file = write_cover(&core, &cover_name);

    core.delete_songs_with_transaction(&[song])
        .expect("el borrado debe completarse");

    assert!(
        !cover_file.exists(),
        "la ruta leída antes del commit debe seguir siendo un candidato"
    );
}
