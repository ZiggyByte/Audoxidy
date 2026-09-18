//! Library data: sorting, filter application, and selection statistics.

use crate::utils::SortColumn;

use super::state::LibraryManager;
use super::types::{AlbumEntry, ArtistGroup, LibraryStats, LibraryViewMode};

impl LibraryManager {
    pub fn sort_songs_static(
        songs: &mut [std::sync::Arc<crate::db::database::SongData>],
        sort_column: Option<SortColumn>,
        sort_ascending: Option<bool>,
    ) {
        if let Some(col_ref) = sort_column {
            let is_asc = sort_ascending.unwrap_or(true);
            songs.sort_by(|a, b| {
                let res = match col_ref {
                    SortColumn::TrackNumber => crate::utils::compare_track_numbers(
                        a.track_number.as_deref(),
                        b.track_number.as_deref(),
                    ),
                    SortColumn::Title => crate::utils::compare_strings_ignore_case(
                        a.title.as_deref().unwrap_or(""),
                        b.title.as_deref().unwrap_or(""),
                    )
                    .then(crate::utils::compare_track_numbers(
                        a.track_number.as_deref(),
                        b.track_number.as_deref(),
                    )),
                    SortColumn::Artist | SortColumn::AlbumArtist => {
                        let art_a = crate::utils::get_effective_artist(a);
                        let art_b = crate::utils::get_effective_artist(b);
                        crate::utils::compare_strings_ignore_case(art_a, art_b)
                            .then(crate::utils::compare_strings_ignore_case(
                                a.release_year.as_deref().unwrap_or(""),
                                b.release_year.as_deref().unwrap_or(""),
                            ))
                            .then(crate::utils::compare_strings_ignore_case(
                                a.album.as_deref().unwrap_or(""),
                                b.album.as_deref().unwrap_or(""),
                            ))
                            .then(crate::utils::compare_track_numbers(
                                a.track_number.as_deref(),
                                b.track_number.as_deref(),
                            ))
                    }
                    SortColumn::Album => {
                        let art_a = crate::utils::get_effective_artist(a);
                        let art_b = crate::utils::get_effective_artist(b);
                        crate::utils::compare_strings_ignore_case(
                            a.album.as_deref().unwrap_or(""),
                            b.album.as_deref().unwrap_or(""),
                        )
                        .then(crate::utils::compare_strings_ignore_case(art_a, art_b))
                        .then(crate::utils::compare_track_numbers(
                            a.track_number.as_deref(),
                            b.track_number.as_deref(),
                        ))
                    }
                    SortColumn::Format => crate::utils::compare_strings_ignore_case(
                        a.format.as_deref().unwrap_or(""),
                        b.format.as_deref().unwrap_or(""),
                    ),
                    SortColumn::Size => a.size.unwrap_or(0).cmp(&b.size.unwrap_or(0)),
                    SortColumn::SampleRate => {
                        a.sample_rate.unwrap_or(0).cmp(&b.sample_rate.unwrap_or(0))
                    }
                    SortColumn::Channels => a.channels.unwrap_or(0).cmp(&b.channels.unwrap_or(0)),
                    SortColumn::Bitrate => {
                        let bitrate = |s: &crate::db::database::SongData| {
                            s.duration_secs
                                .filter(|d| *d > 0.0)
                                .map(|d| s.size.unwrap_or(0) as f64 * 8.0 / (d * 1000.0))
                                .unwrap_or(0.0)
                        };
                        bitrate(&**a)
                            .partial_cmp(&bitrate(&**b))
                            .unwrap_or(std::cmp::Ordering::Equal)
                    }
                    SortColumn::Genre => {
                        let art_a = crate::utils::get_effective_artist(a);
                        let art_b = crate::utils::get_effective_artist(b);
                        crate::utils::compare_strings_ignore_case(
                            a.genre.as_deref().unwrap_or(""),
                            b.genre.as_deref().unwrap_or(""),
                        )
                        .then(crate::utils::compare_strings_ignore_case(art_a, art_b))
                        .then(crate::utils::compare_strings_ignore_case(
                            a.album.as_deref().unwrap_or(""),
                            b.album.as_deref().unwrap_or(""),
                        ))
                        .then(crate::utils::compare_track_numbers(
                            a.track_number.as_deref(),
                            b.track_number.as_deref(),
                        ))
                    }
                    SortColumn::Year => {
                        let art_a = crate::utils::get_effective_artist(a);
                        let art_b = crate::utils::get_effective_artist(b);
                        crate::utils::compare_strings_ignore_case(
                            a.release_year.as_deref().unwrap_or(""),
                            b.release_year.as_deref().unwrap_or(""),
                        )
                        .then(crate::utils::compare_strings_ignore_case(art_a, art_b))
                        .then(crate::utils::compare_strings_ignore_case(
                            a.album.as_deref().unwrap_or(""),
                            b.album.as_deref().unwrap_or(""),
                        ))
                        .then(crate::utils::compare_track_numbers(
                            a.track_number.as_deref(),
                            b.track_number.as_deref(),
                        ))
                    }
                    SortColumn::Duration => {
                        let dur_a = a.duration_secs.unwrap_or(0.0);
                        let dur_b = b.duration_secs.unwrap_or(0.0);
                        dur_a
                            .partial_cmp(&dur_b)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    }
                    SortColumn::BitDepth => a.bit_depth.unwrap_or(0).cmp(&b.bit_depth.unwrap_or(0)),
                    SortColumn::AlbumCard => std::cmp::Ordering::Equal,
                    SortColumn::AlbumThumbnail => std::cmp::Ordering::Equal,
                };
                if is_asc { res } else { res.reverse() }
            });
        } else {
            songs.sort_by(|a, b| crate::utils::compare_songs_for_listing(a, b));
        }
    }

    pub fn sort_songs(&self, songs: &mut [std::sync::Arc<crate::db::database::SongData>]) {
        Self::sort_songs_static(songs, self.sort_column, self.sort_ascending);
    }

    pub fn sort_albums_static(
        albums: &mut [AlbumEntry],
        sort_column: Option<SortColumn>,
        sort_ascending: Option<bool>,
    ) {
        if let Some(col_ref) = sort_column {
            let is_asc = sort_ascending.unwrap_or(true);
            albums.sort_by(|a, b| {
                let res = match col_ref {
                    SortColumn::Album => {
                        crate::utils::compare_strings_ignore_case(&a.title, &b.title)
                            .then(crate::utils::compare_strings_ignore_case(
                                &a.artist, &b.artist,
                            ))
                            .then(crate::utils::compare_strings_ignore_case(&a.year, &b.year))
                    }
                    SortColumn::Artist | SortColumn::AlbumArtist => {
                        crate::utils::compare_albums_for_listing(
                            &a.artist, &a.year, &a.title, &b.artist, &b.year, &b.title,
                        )
                    }
                    SortColumn::Genre => {
                        crate::utils::compare_strings_ignore_case(&a.genre, &b.genre)
                            .then(crate::utils::compare_strings_ignore_case(
                                &a.artist, &b.artist,
                            ))
                            .then(crate::utils::compare_strings_ignore_case(&a.year, &b.year))
                            .then(crate::utils::compare_strings_ignore_case(
                                &a.title, &b.title,
                            ))
                    }
                    SortColumn::Year => crate::utils::compare_strings_ignore_case(&a.year, &b.year)
                        .then(crate::utils::compare_strings_ignore_case(
                            &a.artist, &b.artist,
                        ))
                        .then(crate::utils::compare_strings_ignore_case(
                            &a.title, &b.title,
                        )),
                    _ => std::cmp::Ordering::Equal,
                };
                if is_asc { res } else { res.reverse() }
            });
        } else {
            // Usar el ordenamiento canónico global de Audoxidy por defecto
            albums.sort_by(|a, b| {
                crate::utils::compare_albums_for_listing(
                    &a.artist, &a.year, &a.title, &b.artist, &b.year, &b.title,
                )
            });
        }
    }

    pub fn sort_albums(&self, albums: &mut [AlbumEntry]) {
        Self::sort_albums_static(albums, self.sort_column, self.sort_ascending);
    }

    /// Ordena todos los datos cargados en memoria basándose en el estado actual de ordenamiento.
    pub fn sort_all_data(&mut self) {
        let sc = self.sort_column;
        let sa = self.sort_ascending;

        if let Some(songs) = self.cached_all_songs.as_mut() {
            Self::sort_songs_static(songs, sc, sa);
        }
        if let Some(albums) = self.cached_albums.as_mut() {
            Self::sort_albums_static(albums, sc, sa);
        }
    }

    /// Actualiza los datos maestros de la biblioteca, re-ordenando y aplicando filtros.
    pub fn update_data(
        &mut self,
        songs: Vec<std::sync::Arc<crate::db::database::SongData>>,
        albums: Vec<AlbumEntry>,
    ) {
        self.cached_all_songs = Some(songs.clone());
        self.cached_albums = Some(albums);
        self.update_processed_data(songs);
    }

    pub fn update_selection_stats(&mut self) {
        let selected_songs = self.get_selected_songs();
        if selected_songs.is_empty() {
            self.selection_stats = None;
            return;
        }

        let mut stats = LibraryStats::default();
        let mut unique_albums = std::collections::HashSet::new();
        let mut unique_artists = std::collections::HashSet::new();

        for song in &selected_songs {
            stats.songs += 1;
            stats.duration_secs += song.duration_secs.unwrap_or(0.0);
            stats.size_bytes += song.size.unwrap_or(0) as f64;

            let art = crate::utils::get_effective_artist(song);
            let alb = song.album.as_deref().unwrap_or("Desconocido");
            unique_artists.insert(art.to_string());
            unique_albums.insert((art.to_string(), alb.to_string()));
        }

        stats.albums = unique_albums.len() as u64;
        stats.artists = unique_artists.len() as u64;
        self.selection_stats = Some(stats);
    }

    /// Genera los parámetros de búsqueda basados en el estado actual de los filtros.
    pub fn get_search_params(&self) -> crate::db::database::LibrarySearchParams {
        crate::db::database::LibrarySearchParams {
            query: if self.search_query.is_empty() {
                None
            } else {
                Some(self.search_query.clone())
            },
            artist: self.filter_artist.clone(),
            album: self.filter_album.clone(),
            genre: self.filter_genre.clone(),
            year: self.filter_year.clone(),
            folder_id: self.filter_folder_id,
        }
    }

    /// Prepara el estado para un nuevo filtrado. En el nuevo modelo Stateless,
    /// esto ya no filtra en RAM, sino que indica que se requiere una consulta a la DB.
    pub fn apply_filter(&mut self) {
        self.invalidate_cache();
        self.artist_last_selection.clear();
        self.search_nonce = self.search_nonce.wrapping_add(1);

        // El filtrado real ahora ocurre de forma asíncrona en App vía SQL.
        // Aquí solo podríamos activar un estado de "loading" si fuera necesario.
    }

    /// Procesa el nuevo vector de canciones filtradas (vía SQL) para reconstruir grupos y grid.
    pub fn update_processed_data(
        &mut self,
        songs: Vec<std::sync::Arc<crate::db::database::SongData>>,
    ) {
        self.invalidate_cache();
        let has_any_filter = !self.search_query.is_empty()
            || self.filter_artist.is_some()
            || self.filter_album.is_some()
            || self.filter_genre.is_some()
            || self.filter_year.is_some()
            || self.filter_folder_id.is_some();

        // 1. Agrupación por Artistas
        let mut groups_map: std::collections::BTreeMap<String, ArtistGroup> =
            std::collections::BTreeMap::new();
        for song in &songs {
            let artist_name = crate::utils::get_effective_artist(song);
            let key = artist_name.to_lowercase();

            let group = groups_map.entry(key).or_insert(ArtistGroup {
                name: artist_name.to_string(),
                albums_count: 0,
                songs_count: 0,
                songs: Vec::new(),
                duration_secs: 0.0,
            });

            group.duration_secs += song.duration_secs.unwrap_or(0.0);
            group.songs.push(song.clone());
        }

        for g in groups_map.values_mut() {
            let mut unique_albums = std::collections::HashSet::new();
            for s in &g.songs {
                if let Some(a) = &s.album {
                    unique_albums.insert(a.clone());
                }
            }
            g.albums_count = unique_albums.len();
            g.songs_count = g.songs.len();
        }

        let groups: Vec<ArtistGroup> = groups_map.into_values().collect();
        self.artist_groups = groups;
        self.filtered_songs = Some(songs);

        // 2. Sincronización del Grid (Álbumes)
        if has_any_filter {
            let mut grid_filtered = Vec::new();
            let mut seen_albums = std::collections::HashSet::new();

            if let Some(all_albums) = &self.cached_albums {
                for group in &self.artist_groups {
                    for song in &group.songs {
                        if let Some(alb_title) = &song.album {
                            let key = (group.name.to_lowercase(), alb_title.trim().to_lowercase());
                            if !seen_albums.contains(&key) {
                                seen_albums.insert(key);

                                if let Some(entry) = all_albums
                                    .iter()
                                    .find(|a| {
                                        a.title.as_str() == &**alb_title && a.artist == group.name
                                    })
                                    .or_else(|| {
                                        all_albums.iter().find(|a| a.title.as_str() == &**alb_title)
                                    })
                                {
                                    let mut unified_entry = entry.clone();
                                    unified_entry.artist = group.name.clone();
                                    grid_filtered.push(unified_entry);
                                }
                            }
                        }
                    }
                }
            }
            self.sort_albums(&mut grid_filtered);
            self.filtered_albums = Some(grid_filtered);
        } else {
            self.filtered_albums = None;
        }

        // 3. Estadísticas
        if let Some(songs) = &self.filtered_songs {
            let total_dur: f64 = songs.iter().map(|s| s.duration_secs.unwrap_or(0.0)).sum();
            let total_size: f64 = songs.iter().map(|s| s.size.unwrap_or(0) as f64).sum();
            let mut unique_albums: std::collections::HashSet<(String, String)> =
                std::collections::HashSet::new();
            let mut unique_artists = std::collections::HashSet::new();
            for s in songs {
                let art = crate::utils::get_effective_artist(s);
                let alb = s.album.as_deref().unwrap_or("Desconocido");
                unique_albums.insert((art.to_string(), alb.to_string()));
                unique_artists.insert(art.to_string());
            }
            self.filter_stats = Some(LibraryStats {
                songs: songs.len() as u64,
                albums: unique_albums.len() as u64,
                artists: unique_artists.len() as u64,
                duration_secs: total_dur,
                size_bytes: total_size,
            });
        }

        self.refresh_expanded_album_songs();
    }

    /// Refresca la lista de canciones del álbum expandido en vista Grid basándose en los datos actuales.
    pub fn refresh_expanded_album_songs(&mut self) {
        if self.view_mode != LibraryViewMode::Grid {
            return;
        }

        if let Some(album_id) = self.expanded_album.clone() {
            // Ahora buscamos en filtered_songs o disparamos consulta específica si fuera necesario.
            // Para simplicidad por ahora, si las canciones están en filtered_songs las usamos.
            if let Some(songs) = &self.filtered_songs {
                if let Some(album_entry) = self.cached_albums.as_ref().and_then(|all| {
                    all.iter().find(|a| {
                        (format!("{}|{}", a.artist, a.id) == album_id) || (a.id == album_id)
                    })
                }) {
                    let mut alb_songs: Vec<_> = songs
                        .iter()
                        .filter(|s| {
                            let s_alb = s.album.as_deref().unwrap_or("Desconocido");
                            let s_art = crate::utils::get_effective_artist(s);
                            s_alb == album_entry.title && s_art == album_entry.artist
                        })
                        .cloned()
                        .collect();

                    if !alb_songs.is_empty() {
                        self.sort_songs(&mut alb_songs);
                        self.expanded_album_songs = Some(alb_songs);
                    } else {
                        self.expanded_album = None;
                        self.expanded_album_songs = None;
                    }
                }
            }
        }
    }
}
