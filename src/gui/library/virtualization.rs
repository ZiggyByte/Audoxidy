//! Library virtualization: unloading, cache invalidation, and visible-element math.

use iced::Task;

use super::cover_cache::{CoverKey, DecodeJob, GridCoverElement, GridRowsMemo};
use super::grid_metrics::{GridMetrics, grid_rows, song_y_in_row};
use super::state::{LIBRARY_SCROLL_ID, LibraryManager};
use super::types::{
    AlbumEntry, LibraryAlbumGroup, LibraryGridElement, LibraryListItem, LibraryViewMode,
    LibraryVirtualArtist, LibraryVirtualRow,
};
use crate::gui::dimensions::{LIST_ROW, LIST_ROW_HEADER};

impl LibraryManager {
    /// Descarga los datos pesados de la RAM por inactividad, conservando un margen de seguridad
    /// Descarga los datos pesados de la RAM si no se están usando
    pub fn unload(&mut self, is_focused: bool) {
        if is_focused {
            // Si está enfocada, mantenemos los datos base (all_songs, albums) para que no desaparezca nada.
            // Solo invalidamos la caché de virtualización de la vista para liberar algo de RAM (se regenera al instante al mover el mouse/scroll)
            self.invalidate_cache();
            return;
        }

        // Condición inteligente: No descargar si la biblioteca es pequeña (menos de 250 álbumes y 2000 canciones)
        // Esto mantiene el reproductor "snappy" para usuarios con colecciones medianas/pequeñas.
        let song_count = self.cached_all_songs.as_ref().map(|s| s.len()).unwrap_or(0);
        let album_count = self.cached_albums.as_ref().map(|a| a.len()).unwrap_or(0);

        // Condición de protección: No descargar nada si la biblioteca es pequeña
        // (menos de 150 álbumes o menos de 700 canciones)
        if album_count < 150 || song_count < 700 {
            return;
        }

        // Si la biblioteca es grande y no está enfocada, realizamos una DESCARGA PARCIAL
        // En lugar de poner None, truncamos a un margen de seguridad (100 álbumes / 500 canciones)
        // Esto evita que la interfaz se vea vacía si el usuario vuelve rápido, pero libera la mayoría de la RAM.
        let mut cleared = false;

        if let Some(albums) = &mut self.cached_albums {
            if albums.len() > 100 {
                tracing::debug!("Audoxidy GC: Cleaning inactive albums from the library");
                albums.truncate(100);
                albums.shrink_to_fit();
                self.filtered_albums = None;
                cleared = true;
            }
        }

        if let Some(songs) = &mut self.cached_all_songs {
            if songs.len() > 500 {
                tracing::debug!("Audoxidy GC: Cleaning inactive songs from the library");
                songs.truncate(500);
                songs.shrink_to_fit();
                self.filtered_songs = None;
                cleared = true;
            }
        }

        if cleared {
            self.invalidate_cache();
            self.artist_groups.clear();
            self.artist_groups.shrink_to_fit();
            self.data_unloaded = true;
        }
    }

    pub fn invalidate_cache(&mut self) {
        self.cached_detailed_view.replace(None);
        self.cached_visible_elements.replace(None);
        self.cached_grid_rows.replace(None);
        // Al invalidar caché por cambio de estructura, el hint de scroll ya no es válido
        self.selected_item_hint = None;
    }

    pub fn invalidate_visible_cache(&mut self) {
        self.cached_visible_elements.replace(None);
        self.cached_grid_rows.replace(None);
    }

    /// Genera la estructura jerárquica para la vista de biblioteca
    /// Este proceso es O(N) y solo se ejecuta cuando cambian los filtros o expansiones.
    pub fn get_view_structure(&self) -> (f32, Vec<LibraryVirtualArtist>) {
        if let Some((h, ref v)) = *self.cached_detailed_view.borrow() {
            return (h, v.clone());
        }

        let groups = &self.artist_groups;
        let mut v_artists = Vec::new();
        let mut total_content_h = 0.0;
        let mut global_song_idx = 0;

        // Crear mapa de búsqueda de álbumes para evitar O(N^2)
        let mut album_hash_map = std::collections::HashMap::new();
        // Album cards need their cover path resolved once per snapshot; album ids
        // are unique, so the map is keyed by id rather than by display title.
        let mut album_path_map: std::collections::HashMap<String, Option<String>> =
            std::collections::HashMap::new();
        // Identity list per (artist, title): more than one id is a title collision.
        let mut album_key_ids: std::collections::HashMap<(String, String), Vec<String>> =
            std::collections::HashMap::new();
        if let Some(albums_cache) = &self.cached_albums {
            for alb in albums_cache {
                album_hash_map.insert((alb.artist.clone(), alb.title.clone()), alb.id.clone());
                album_path_map
                    .entry(alb.id.clone())
                    .or_insert_with(|| alb.cover_path.clone());

                let ids = album_key_ids
                    .entry((alb.artist.clone(), alb.title.clone()))
                    .or_default();
                if !ids.contains(&alb.id) {
                    ids.push(alb.id.clone());
                }
            }
        }

        let row_height = if self.view_mode == LibraryViewMode::ThumbnailList {
            LIST_ROW
        } else {
            LIST_ROW_HEADER
        };
        let header_h = if self.view_mode == LibraryViewMode::ThumbnailList {
            LIST_ROW
        } else {
            LIST_ROW_HEADER
        };

        for group in groups {
            let is_artist_collapsed = self.collapsed_artists.contains(&group.name);
            let mut albums_map: Vec<LibraryAlbumGroup> = Vec::new();

            // Agrupar canciones por álbum conservando el orden O(N)
            for song in &group.songs {
                let alb_name = song
                    .album
                    .as_ref()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "Desconocido".to_string());

                if let Some(last_alb) = albums_map.last_mut() {
                    if last_alb.album_name == alb_name {
                        last_alb.duration_secs += song.duration_secs.unwrap_or(0.0);
                        last_alb.songs.push((song.clone(), global_song_idx));
                        global_song_idx += 1;
                        continue;
                    }
                }

                let album_key = (group.name.clone(), alb_name.clone());
                let album_hash = album_hash_map
                    .get(&album_key)
                    .cloned()
                    .unwrap_or_else(|| alb_name.clone());

                // Two albums with the same title and artist cannot be told apart by
                // the grouping key, so the card uses the already-resolved path of
                // its first song instead of guessing the album row's path.
                let ambiguous_title = album_key_ids
                    .get(&album_key)
                    .is_some_and(|ids| ids.len() > 1);
                if ambiguous_title {
                    tracing::debug!(
                        artist = %group.name,
                        album = %alb_name,
                        "Se detectó un título de álbum ambiguo; se usa la ruta de la primera canción"
                    );
                }

                let cover_path = if ambiguous_title {
                    song.compressed_cached_cover_root
                        .as_deref()
                        .map(str::to_owned)
                } else {
                    album_path_map
                        .get(&album_hash)
                        .cloned()
                        .flatten()
                        .or_else(|| {
                            song.compressed_cached_cover_root
                                .as_deref()
                                .map(str::to_owned)
                        })
                };

                albums_map.push(LibraryAlbumGroup {
                    album_name: alb_name,
                    album_hash,
                    cover_path,
                    songs: vec![(song.clone(), global_song_idx)],
                    duration_secs: song.duration_secs.unwrap_or(0.0),
                });
                global_song_idx += 1;
            }

            let mut artist_h = header_h;
            if !is_artist_collapsed {
                for alb in &albums_map {
                    let composite_id = format!("{}|{}", group.name, alb.album_hash);
                    let is_album_expanded = !self.collapsed_albums.contains(&composite_id);

                    let album_header_h = if self.view_mode == LibraryViewMode::ThumbnailList {
                        LIST_ROW
                    } else {
                        LIST_ROW_HEADER
                    };
                    let card_h: f32 =
                        if self.view_mode == LibraryViewMode::DetailedList && is_album_expanded {
                            323.0
                        } else {
                            0.0
                        };
                    let right_h = if is_album_expanded {
                        album_header_h + alb.songs.len() as f32 * row_height
                    } else {
                        album_header_h
                    };

                    let block_h = if self.view_mode == LibraryViewMode::DetailedList {
                        card_h.max(right_h) + 10.0
                    } else {
                        right_h
                    };

                    artist_h += block_h;
                }
            }

            v_artists.push(LibraryVirtualArtist {
                artist_name: group.name.clone(),
                albums_count: group.albums_count,
                songs_count: group.songs_count,
                duration_secs: group.duration_secs,
                is_collapsed: is_artist_collapsed,
                top_y: total_content_h,
                height: artist_h,
                albums: albums_map,
            });

            total_content_h += artist_h;
        }

        self.cached_detailed_view
            .replace(Some((total_content_h, v_artists.clone())));
        (total_content_h, v_artists)
    }

    /// Obtiene los elementos visibles según el viewport actual (Virtualización)
    #[tracing::instrument(skip(self))]
    pub fn get_visible_elements(
        &self,
    ) -> (
        f32,
        f32,
        f32,
        Vec<LibraryVirtualRow>,
        Option<(String, bool, usize, usize, f64)>,
    ) {
        if let Some((ts, bs, th, ref rows, ref sticky)) = *self.cached_visible_elements.borrow() {
            return (ts, bs, th, rows.clone(), sticky.clone());
        }

        let viewport_h = self
            .last_viewport
            .as_ref()
            .map(|v| v.height)
            .unwrap_or(800.0);
        let result = self.visible_rows_at(self.scroll_offset.y, viewport_h);

        self.cached_visible_elements.replace(Some((
            result.0,
            result.1,
            result.2,
            result.3.clone(),
            result.4.clone(),
        )));

        result
    }

    /// Rows visible at an arbitrary viewport, without touching the row memo.
    ///
    /// The layout path reads the memoized rows; the cover request path needs the
    /// rows for a viewport that is not the one on screen (the look-ahead band), so
    /// both go through this builder instead of the request path forcing a memo
    /// write for an offset nothing is rendering.
    // The tuple is the row-window contract the memoized getter returns too; a type
    // alias would only rename the same five elements.
    #[allow(clippy::type_complexity)]
    pub fn visible_rows_at(
        &self,
        viewport_y: f32,
        viewport_h: f32,
    ) -> (
        f32,
        f32,
        f32,
        Vec<LibraryVirtualRow>,
        Option<(String, bool, usize, usize, f64)>,
    ) {
        let (total_content_h, v_artists) = self.get_view_structure();
        let view_min_raw = viewport_y;

        let max_scroll = (total_content_h - viewport_h).max(0.0);
        let view_min = view_min_raw.min(max_scroll);
        let view_max = view_min + viewport_h;

        // Margen dinámico = 50% del viewport (25% en low-resource)
        let viewport_margin_ratio: f32 = if crate::utils::is_low_resource() {
            0.25
        } else {
            0.5
        };
        let margin = (viewport_h * viewport_margin_ratio).max(50.0);
        let render_min = view_min - margin;
        let render_max = view_max + margin;

        let mut top_space = 0.0;
        let mut bottom_space = 0.0;
        let mut visible_elements = Vec::new();
        let mut sticky_artist_info = None;

        let header_h = if self.view_mode == LibraryViewMode::ThumbnailList {
            LIST_ROW
        } else {
            LIST_ROW_HEADER
        };
        let album_header_h = if self.view_mode == LibraryViewMode::ThumbnailList {
            LIST_ROW
        } else {
            LIST_ROW_HEADER
        };
        let row_height = if self.view_mode == LibraryViewMode::ThumbnailList {
            LIST_ROW
        } else {
            LIST_ROW_HEADER
        };

        for va in v_artists {
            let a_end = va.top_y + va.height;

            // Sticky Header: umbral +31px (como en widgets.rs)
            let sticky_threshold = header_h - 1.0;
            if va.top_y <= view_min + sticky_threshold {
                sticky_artist_info = Some((
                    va.artist_name.clone(),
                    va.is_collapsed,
                    va.albums_count,
                    va.songs_count,
                    va.duration_secs,
                ));
            }

            if a_end < render_min {
                top_space += va.height;
            } else if va.top_y > render_max {
                bottom_space += va.height;
            } else {
                // Header del Artista
                let ah_end = va.top_y + header_h;
                if ah_end >= render_min && va.top_y <= render_max {
                    visible_elements.push(LibraryVirtualRow::ArtistHeader {
                        name: va.artist_name.clone(),
                        is_collapsed: va.is_collapsed,
                        albums_count: va.albums_count,
                        songs_count: va.songs_count,
                        duration_secs: va.duration_secs,
                    });
                } else if ah_end < render_min {
                    top_space += header_h;
                } else {
                    bottom_space += header_h;
                }

                if !va.is_collapsed {
                    let mut current_y = va.top_y + header_h;
                    for alb in &va.albums {
                        let composite_id = format!("{}|{}", va.artist_name, alb.album_hash);
                        let is_album_expanded = !self.collapsed_albums.contains(&composite_id);
                        let card_h: f32 = if self.view_mode == LibraryViewMode::DetailedList
                            && is_album_expanded
                        {
                            323.0
                        } else {
                            0.0
                        };
                        let right_h = if is_album_expanded {
                            album_header_h + alb.songs.len() as f32 * row_height
                        } else {
                            album_header_h
                        };

                        let block_h = if self.view_mode == LibraryViewMode::DetailedList {
                            card_h.max(right_h) + 10.0
                        } else {
                            right_h
                        };

                        let block_end = current_y + block_h;

                        if block_end >= render_min && current_y <= render_max {
                            // En modo no detallado, los headers de álbum y canciones se pueden virtualizar individualmente
                            if self.view_mode != LibraryViewMode::DetailedList {
                                // Header Álbum
                                if current_y + album_header_h >= render_min
                                    && current_y <= render_max
                                {
                                    visible_elements.push(LibraryVirtualRow::AlbumBlock {
                                        album_name: alb.album_name.clone(),
                                        album_hash: alb.album_hash.clone(),
                                        artist_name: va.artist_name.clone(),
                                        cover_path: alb.cover_path.clone(),
                                        genre: String::new(),
                                        year: String::new(), // No se usan en este modo
                                        is_expanded: is_album_expanded,
                                        songs: Vec::new(),
                                        songs_count: alb.songs.len(),
                                        duration_secs: alb.duration_secs,
                                    });
                                } else if current_y + album_header_h < render_min {
                                    top_space += album_header_h;
                                } else {
                                    bottom_space += album_header_h;
                                }

                                if is_album_expanded {
                                    let mut song_y = current_y + album_header_h;
                                    for (song, song_i) in &alb.songs {
                                        if song_y + row_height >= render_min && song_y <= render_max
                                        {
                                            visible_elements.push(LibraryVirtualRow::SimpleSong {
                                                song: song.clone(),
                                                global_idx: *song_i,
                                            });
                                        } else if song_y + row_height < render_min {
                                            top_space += row_height;
                                        } else {
                                            bottom_space += row_height;
                                        }
                                        song_y += row_height;
                                    }
                                }
                            } else {
                                // Modo Detallado: El bloque completo se renderiza (es mejor así por el layout de tarjeta)
                                let genre = alb
                                    .songs
                                    .first()
                                    .and_then(|(s, _)| s.genre.as_ref().map(|s| s.to_string()))
                                    .unwrap_or_default();
                                let year = alb
                                    .songs
                                    .first()
                                    .and_then(|(s, _)| {
                                        s.release_year.as_ref().map(|s| s.to_string())
                                    })
                                    .unwrap_or_default();
                                visible_elements.push(LibraryVirtualRow::AlbumBlock {
                                    album_name: alb.album_name.clone(),
                                    album_hash: alb.album_hash.clone(),
                                    artist_name: va.artist_name.clone(),
                                    cover_path: alb.cover_path.clone(),
                                    genre,
                                    year,
                                    is_expanded: is_album_expanded,
                                    songs: alb.songs.clone(),
                                    songs_count: alb.songs.len(),
                                    duration_secs: alb.duration_secs,
                                });
                            }
                        } else if block_end < render_min {
                            top_space += block_h;
                        } else {
                            bottom_space += block_h;
                        }
                        current_y += block_h;
                    }
                }
            }
        }

        (
            top_space,
            bottom_space,
            total_content_h,
            visible_elements,
            sticky_artist_info,
        )
    }

    /// Rendered song count of the expanded album.
    ///
    /// Counts only the songs that pass the active search, which is exactly the
    /// list the renderer receives, and returns zero while the song list is still
    /// loading so no space is reserved for a row that does not exist.
    pub fn rendered_expanded_song_count(&self) -> usize {
        let Some(songs) = self.expanded_album_songs.as_ref() else {
            return 0;
        };

        let query = self.search_query.trim();
        if query.is_empty() {
            songs.len()
        } else {
            songs
                .iter()
                .filter(|song| crate::utils::song_matches_search(song, query))
                .count()
        }
    }

    /// Row index that hosts the expanded album inside `albums`.
    pub fn expanded_album_row(&self, albums: &[AlbumEntry], columns: usize) -> Option<usize> {
        let expanded = self.expanded_album.as_deref()?;
        let (artist, album_id) = expanded.split_once('|')?;
        let album_index = albums
            .iter()
            .position(|album| album.artist == artist && album.id == album_id)?;

        Some(album_index / columns)
    }

    /// Index of the expanded album in the rendered album list and its rendered
    /// song count.
    ///
    /// The identity is the composite id the expansion stores, so the anchor
    /// survives a geometry change untouched. Both halves come from the
    /// filtered-or-cached list and the search-filtered song list, which are the
    /// ones the renderer projects from; the album scan allocates nothing.
    pub fn expanded_album_anchor(&self) -> Option<(usize, usize)> {
        let expanded = self.expanded_album.as_deref()?;
        let (artist, album_id) = expanded.split_once('|')?;
        let albums = self
            .filtered_albums
            .as_ref()
            .or(self.cached_albums.as_ref())?;
        let album_index = albums
            .iter()
            .position(|album| album.artist == artist && album.id == album_id)?;

        Some((album_index, self.rendered_expanded_song_count()))
    }

    /// Calcula los elementos visibles en la cuadrícula para el modo Grid, con virtualización avanzada.
    #[tracing::instrument(skip(self))]
    pub fn get_visible_grid_elements(
        &self,
        metrics: &GridMetrics,
        viewport_y: f32,
        viewport_h: f32,
    ) -> (f32, f32, Vec<LibraryGridElement>) {
        let albums_to_show = self
            .filtered_albums
            .as_ref()
            .or(self.cached_albums.as_ref());

        let albums = if let Some(a) = albums_to_show {
            a
        } else {
            return (0.0, 0.0, Vec::new());
        };

        let expanded_row = self.expanded_album_row(albums, metrics.columns);
        let rendered_songs = self.rendered_expanded_song_count();
        let spans = grid_rows(metrics, albums.len(), expanded_row, rendered_songs);

        // Margen dinámico en grid = 50% del viewport (25% en low-resource)
        let grid_margin_ratio: f32 = if crate::utils::is_low_resource() {
            0.25
        } else {
            0.5
        };
        let lazy_margin = (viewport_h * grid_margin_ratio).max(100.0);
        let render_min = viewport_y - lazy_margin;
        let render_max = viewport_y + viewport_h + lazy_margin;

        let mut top_space = 0.0;
        let mut bottom_space = 0.0;
        let mut visible_elements = Vec::new();

        for (row_index, span) in spans.iter().enumerate() {
            let row_end = span.top + span.height;
            if row_end < render_min {
                top_space += span.height;
                continue;
            }
            if span.top > render_max {
                bottom_space += span.height;
                continue;
            }

            // Only the rows inside the render window are materialised; the rows
            // above and below are represented by their accumulated height.
            let row_start = row_index * metrics.columns;
            let row_limit = (row_start + metrics.columns).min(albums.len());

            visible_elements.push(LibraryGridElement::AlbumRow {
                albums: albums[row_start..row_limit].to_vec(),
                active_expansion_id: if span.expanded {
                    self.expanded_album.clone()
                } else {
                    None
                },
            });

            if span.expanded
                && let Some(songs) = self.expanded_album_songs.as_ref()
            {
                let query = self.search_query.trim();
                let mut v_songs = Vec::new();
                let mut songs_top_space = 0.0;
                let mut songs_bottom_space = 0.0;
                let mut rendered_index = 0;

                for (idx, song) in songs.iter().enumerate() {
                    if !query.is_empty() && !crate::utils::song_matches_search(song, query) {
                        continue;
                    }

                    let song_y = song_y_in_row(metrics, span.top, rendered_index);
                    if song_y + LIST_ROW_HEADER >= render_min && song_y <= render_max {
                        v_songs.push((song.clone(), idx));
                    } else if song_y + LIST_ROW_HEADER < render_min {
                        songs_top_space += LIST_ROW_HEADER;
                    } else {
                        songs_bottom_space += LIST_ROW_HEADER;
                    }
                    rendered_index += 1;
                }

                if !v_songs.is_empty() || songs_top_space > 0.0 || songs_bottom_space > 0.0 {
                    visible_elements.push(LibraryGridElement::ExpandedSongs {
                        album_id: self.expanded_album.clone().unwrap_or_default(),
                        songs: v_songs,
                        top_space: songs_top_space,
                        bottom_space: songs_bottom_space,
                    });
                }
            }
        }

        (top_space, bottom_space, visible_elements)
    }

    /// Returns the grid window with one handle already resolved per album card.
    ///
    /// A memo hit for the same geometry, viewport and bucket clones the stored
    /// row slice and returns the stored spacers, so the layout pass opens no file
    /// and performs no cover lookup. A miss rebuilds every element from the
    /// snapshot, resolving one borrowed lookup per card and keeping the expanded
    /// album's song block verbatim.
    pub fn get_visible_grid_rows(
        &self,
        metrics: &GridMetrics,
        viewport_y: f32,
        viewport_h: f32,
    ) -> (f32, f32, std::rc::Rc<[GridCoverElement]>) {
        let bucket = crate::utils::covers::library_cover_bucket(
            metrics.cover_size,
            self.scale_factor,
            crate::utils::covers::cover_native_size(),
        );

        let memo_hit = {
            let memo = self.cached_grid_rows.borrow();
            memo.as_ref()
                .filter(|memo| {
                    memo.metrics == *metrics
                        && memo.viewport_y == viewport_y
                        && memo.viewport_h == viewport_h
                        && memo.bucket == bucket
                })
                .map(|memo| {
                    (
                        memo.top_space,
                        memo.bottom_space,
                        std::rc::Rc::clone(&memo.elements),
                    )
                })
        };

        if let Some(hit) = memo_hit {
            return hit;
        }

        let (top_space, bottom_space, elements) =
            self.get_visible_grid_elements(metrics, viewport_y, viewport_h);

        let mut rows = Vec::with_capacity(elements.len());
        for element in elements {
            rows.push(match element {
                LibraryGridElement::AlbumRow {
                    albums,
                    active_expansion_id,
                } => {
                    let handles = albums
                        .iter()
                        .map(|album| {
                            album
                                .cover_path
                                .as_deref()
                                .filter(|path| !path.is_empty())
                                .and_then(|path| self.cover_store.get_path(path, bucket as u16))
                        })
                        .collect();

                    GridCoverElement::AlbumRow {
                        albums,
                        handles,
                        active_expansion_id,
                    }
                }
                LibraryGridElement::ExpandedSongs {
                    album_id,
                    songs,
                    top_space,
                    bottom_space,
                } => GridCoverElement::ExpandedSongs {
                    album_id,
                    songs,
                    top_space,
                    bottom_space,
                },
            });
        }

        let elements: std::rc::Rc<[GridCoverElement]> = std::rc::Rc::from(rows);
        self.cached_grid_rows.replace(Some(GridRowsMemo {
            metrics: *metrics,
            viewport_y,
            viewport_h,
            bucket,
            top_space,
            bottom_space,
            elements: std::rc::Rc::clone(&elements),
        }));

        (top_space, bottom_space, elements)
    }

    /// Returns the handle stored for a cover path at the rendered logical size.
    ///
    /// The bucket is derived here from the display scale so a read cannot pick a
    /// different bucket from the request that filled the entry. The lookup is
    /// borrowed: no key allocation and no lock. A thumbnail row reads through
    /// this rule; an album card goes through [`Self::album_cover_handle`], which
    /// applies the library minimum.
    pub fn cover_handle(
        &self,
        path: Option<&str>,
        logical_px: f32,
    ) -> Option<iced::widget::image::Handle> {
        let bucket = crate::utils::covers::cover_bucket(
            logical_px,
            self.scale_factor,
            crate::utils::covers::cover_native_size(),
        );

        self.stored_cover_handle(path, bucket)
    }

    /// Returns the handle stored for an album card's cover path.
    ///
    /// Album covers decode at no less than
    /// [`MIN_LIBRARY_COVER_BUCKET`](crate::utils::covers::MIN_LIBRARY_COVER_BUCKET)
    /// pixels, so a card on a 1x display is drawn from a sharp texture instead of
    /// the smallest bucket that fits it. The request path resolves the same
    /// bucket, so this read cannot miss an entry the window just filled.
    pub fn album_cover_handle(
        &self,
        path: Option<&str>,
        logical_px: f32,
    ) -> Option<iced::widget::image::Handle> {
        let bucket = crate::utils::covers::library_cover_bucket(
            logical_px,
            self.scale_factor,
            crate::utils::covers::cover_native_size(),
        );

        self.stored_cover_handle(path, bucket)
    }

    /// Borrowed handle lookup shared by the two bucket rules.
    fn stored_cover_handle(
        &self,
        path: Option<&str>,
        bucket: u32,
    ) -> Option<iced::widget::image::Handle> {
        let path = path.filter(|path| !path.is_empty())?;

        self.cover_store.get_path(path, bucket as u16)
    }

    /// Decode bucket the current view mode renders its covers at.
    ///
    /// The album modes resolve through
    /// [`library_cover_bucket`](crate::utils::covers::library_cover_bucket): the
    /// grid follows the measured card edge raised to the album minimum and the
    /// detailed list its 220 px card (`250 - 30`). The thumbnail list keeps its
    /// 32 px display bucket, because that is the edge its rows draw at.
    pub fn window_bucket(&self) -> u32 {
        let native = crate::utils::covers::cover_native_size();
        match self.view_mode {
            LibraryViewMode::DetailedList => {
                crate::utils::covers::library_cover_bucket(220.0, self.scale_factor, native)
            }
            LibraryViewMode::ThumbnailList => {
                crate::utils::covers::cover_bucket(32.0, self.scale_factor, native)
            }
            _ => crate::utils::covers::library_cover_bucket(
                self.grid_metrics.get().cover_size,
                self.scale_factor,
                native,
            ),
        }
    }

    /// Decode jobs for the cover paths the current viewport shows.
    ///
    /// The job set follows the view mode, so no mode requests a bucket its rows do
    /// not read: the grid asks for its card edge raised to the album minimum, the
    /// detailed list for the album card paths and the thumbnail list for the
    /// per-song thumbnails at their 32 px edge. A path the negative set already
    /// knows to fail is skipped.
    pub fn window_jobs(&self, viewport_y: f32, viewport_h: f32) -> Vec<DecodeJob> {
        match self.view_mode {
            LibraryViewMode::Grid => self.grid_window_jobs(viewport_y, viewport_h),
            LibraryViewMode::DetailedList | LibraryViewMode::ThumbnailList => {
                self.list_window_jobs(viewport_y, viewport_h, self.window_bucket())
            }
            LibraryViewMode::SimpleList => Vec::new(),
        }
    }

    /// Jobs for one grid viewport at the album bucket the grid cards read.
    fn grid_window_jobs(&self, viewport_y: f32, viewport_h: f32) -> Vec<DecodeJob> {
        let metrics = self.grid_metrics.get();
        let bucket = crate::utils::covers::library_cover_bucket(
            metrics.cover_size,
            self.scale_factor,
            crate::utils::covers::cover_native_size(),
        );
        let (_, _, elements) = self.get_visible_grid_elements(&metrics, viewport_y, viewport_h);

        let mut jobs = Vec::new();
        for element in &elements {
            let LibraryGridElement::AlbumRow { albums, .. } = element else {
                continue;
            };

            for album in albums {
                let Some(path) = album.cover_path.as_deref().filter(|path| !path.is_empty()) else {
                    continue;
                };
                if crate::utils::covers::is_cover_negative(path) {
                    continue;
                }

                jobs.push(DecodeJob {
                    key: CoverKey {
                        path: std::sync::Arc::from(path),
                        bucket: bucket as u16,
                    },
                    path: std::path::PathBuf::from(path),
                    bucket,
                });
            }
        }

        jobs
    }

    /// Jobs for one list viewport at the bucket its rows render.
    ///
    /// The detailed list draws the album card's cover and the thumbnail list draws
    /// each song's own cover, which is already the resolved path the row shows.
    fn list_window_jobs(&self, viewport_y: f32, viewport_h: f32, bucket: u32) -> Vec<DecodeJob> {
        let (_, _, _, rows, _) = self.visible_rows_at(viewport_y, viewport_h);

        let mut jobs = Vec::new();
        for row in &rows {
            let path = match (self.view_mode, row) {
                (
                    LibraryViewMode::DetailedList,
                    LibraryVirtualRow::AlbumBlock { cover_path, .. },
                ) => cover_path.as_deref(),
                (LibraryViewMode::ThumbnailList, LibraryVirtualRow::SimpleSong { song, .. }) => {
                    song.compressed_cached_cover_root.as_deref()
                }
                _ => None,
            };

            let Some(path) = path.filter(|path| !path.is_empty()) else {
                continue;
            };
            if crate::utils::covers::is_cover_negative(path) {
                continue;
            }

            jobs.push(DecodeJob {
                key: CoverKey {
                    path: std::sync::Arc::from(path),
                    bucket: bucket as u16,
                },
                path: std::path::PathBuf::from(path),
                bucket,
            });
        }

        jobs
    }

    /// Obtiene el rango Y (y, height) de un ítem en la biblioteca virtualizada
    pub fn get_item_y_range(&self, item: &LibraryListItem) -> Option<(f32, f32)> {
        let (_total_h, v_artists) = self.get_view_structure();
        let header_h = if self.view_mode == LibraryViewMode::ThumbnailList {
            LIST_ROW
        } else {
            LIST_ROW_HEADER
        };
        let album_header_h = if self.view_mode == LibraryViewMode::ThumbnailList {
            LIST_ROW
        } else {
            LIST_ROW_HEADER
        };
        let row_height = if self.view_mode == LibraryViewMode::ThumbnailList {
            LIST_ROW
        } else {
            LIST_ROW_HEADER
        };

        match item {
            LibraryListItem::Artist(name) => {
                v_artists
                    .iter()
                    .find(|va| va.artist_name == *name)
                    .map(|va| (va.top_y, header_h)) // El rango del header
            }
            LibraryListItem::Album(composite_id) => {
                // composite_id format: "Artist|AlbumHash"
                let parts: Vec<&str> = composite_id.split('|').collect();
                if parts.len() < 2 {
                    return None;
                }
                let artist_name = parts[0];
                let album_hash = parts[1];

                let va = v_artists.iter().find(|va| va.artist_name == artist_name)?;
                let mut current_y = va.top_y + header_h;

                for alb in &va.albums {
                    let is_album_expanded = !self
                        .collapsed_albums
                        .contains(&format!("{}|{}", va.artist_name, alb.album_hash));
                    let card_h: f32 =
                        if self.view_mode == LibraryViewMode::DetailedList && is_album_expanded {
                            323.0
                        } else {
                            0.0
                        };
                    let right_h = if is_album_expanded {
                        album_header_h + alb.songs.len() as f32 * row_height
                    } else {
                        album_header_h
                    };
                    let block_h = if self.view_mode == LibraryViewMode::DetailedList {
                        card_h.max(right_h) + 10.0
                    } else {
                        right_h
                    };

                    if alb.album_hash == album_hash {
                        return Some((current_y, album_header_h)); // Retornamos el header del álbum
                    }
                    current_y += block_h;
                }
                None
            }
            LibraryListItem::Song(song_id) => {
                for va in &v_artists {
                    if va.is_collapsed {
                        continue;
                    }
                    let mut current_y = va.top_y + header_h;
                    for alb in &va.albums {
                        let composite_id = format!("{}|{}", va.artist_name, alb.album_hash);
                        let is_album_expanded = !self.collapsed_albums.contains(&composite_id);

                        let card_h: f32 = if self.view_mode == LibraryViewMode::DetailedList
                            && is_album_expanded
                        {
                            323.0
                        } else {
                            0.0
                        };
                        let right_h = if is_album_expanded {
                            album_header_h + alb.songs.len() as f32 * row_height
                        } else {
                            album_header_h
                        };
                        let block_h = if self.view_mode == LibraryViewMode::DetailedList {
                            card_h.max(right_h) + 10.0
                        } else {
                            right_h
                        };

                        if is_album_expanded {
                            let mut song_y = current_y + album_header_h;
                            for (song, _) in &alb.songs {
                                if song.id == *song_id {
                                    return Some((song_y, row_height));
                                }
                                song_y += row_height;
                            }
                        }
                        current_y += block_h;
                    }
                }
                None
            }
        }
    }

    /// Maneja la navegación por teclado unificada entre cabeceras y canciones.
    /// Devuelve (NuevaCabecera, NuevoIndiceCanción, FocoEnCanción).
    pub fn get_row_height(&self) -> f32 {
        match self.view_mode {
            LibraryViewMode::ThumbnailList => LIST_ROW,
            _ => LIST_ROW_HEADER,
        }
    }

    pub fn get_header_height(&self) -> f32 {
        match self.view_mode {
            LibraryViewMode::ThumbnailList => LIST_ROW,
            _ => LIST_ROW_HEADER,
        }
    }

    pub fn is_list_mode(&self) -> bool {
        self.view_mode == LibraryViewMode::SimpleList
            || self.view_mode == LibraryViewMode::DetailedList
            || self.view_mode == LibraryViewMode::ThumbnailList
    }

    pub fn get_visible_items(&self) -> Vec<(LibraryListItem, iced::Rectangle)> {
        let mut items = Vec::new();
        let header_h: f32 = if self.view_mode == LibraryViewMode::ThumbnailList {
            LIST_ROW
        } else {
            LIST_ROW_HEADER
        };
        let album_header_h: f32 = if self.view_mode == LibraryViewMode::ThumbnailList {
            LIST_ROW
        } else {
            LIST_ROW_HEADER
        };
        let row_height: f32 = if self.view_mode == LibraryViewMode::ThumbnailList {
            LIST_ROW
        } else {
            LIST_ROW_HEADER
        };
        let lib_width = self.library_area_width.max(100.0);

        if self.is_list_mode() {
            let (_total_h, v_artists) = self.get_view_structure();
            for va in v_artists {
                items.push((
                    LibraryListItem::Artist(va.artist_name.clone()),
                    iced::Rectangle::new(
                        iced::Point::new(0.0, va.top_y),
                        iced::Size::new(lib_width, header_h),
                    ),
                ));

                if !va.is_collapsed {
                    let mut current_y = va.top_y + header_h;
                    for alb in va.albums {
                        let composite_id = format!("{}|{}", va.artist_name, alb.album_hash);
                        let is_album_expanded = !self.collapsed_albums.contains(&composite_id);

                        let card_h: f32 = if self.view_mode == LibraryViewMode::DetailedList
                            && is_album_expanded
                        {
                            323.0
                        } else {
                            0.0
                        };
                        let right_h = if is_album_expanded {
                            album_header_h + alb.songs.len() as f32 * row_height
                        } else {
                            album_header_h
                        };
                        let block_h = if self.view_mode == LibraryViewMode::DetailedList {
                            card_h.max(right_h) + 10.0
                        } else {
                            right_h
                        };

                        items.push((
                            LibraryListItem::Album(composite_id),
                            iced::Rectangle::new(
                                iced::Point::new(0.0, current_y),
                                iced::Size::new(lib_width, album_header_h),
                            ),
                        ));

                        if is_album_expanded {
                            let mut song_y = current_y + album_header_h;
                            for (song, _) in alb.songs {
                                items.push((
                                    LibraryListItem::Song(song.id),
                                    iced::Rectangle::new(
                                        iced::Point::new(0.0, song_y),
                                        iced::Size::new(lib_width, row_height),
                                    ),
                                ));
                                song_y += row_height;
                            }
                        }
                        current_y += block_h;
                    }
                }
            }
        } else {
            // Modo Grid: Los elementos son álbumes y opcionalmente canciones si está expandido
            let albums_to_show = self
                .filtered_albums
                .as_ref()
                .or(self.cached_albums.as_ref());

            if let Some(albums) = albums_to_show {
                let metrics = self.grid_metrics.get();
                let expanded_row = self.expanded_album_row(albums, metrics.columns);
                let rendered_songs = self.rendered_expanded_song_count();
                let row_width = metrics.card_width * metrics.columns as f32;

                for (row_index, span) in
                    grid_rows(&metrics, albums.len(), expanded_row, rendered_songs)
                        .into_iter()
                        .enumerate()
                {
                    let row_start = row_index * metrics.columns;

                    for column in 0..metrics.columns {
                        let Some(album_entry) = albums.get(row_start + column) else {
                            break;
                        };
                        let composite_id = format!("{}|{}", album_entry.artist, album_entry.id);

                        items.push((
                            LibraryListItem::Album(composite_id),
                            iced::Rectangle::new(
                                iced::Point::new(column as f32 * metrics.card_width, span.top),
                                iced::Size::new(metrics.card_width, metrics.card_height),
                            ),
                        ));
                    }

                    if span.expanded
                        && let Some(songs) = self.expanded_album_songs.as_ref()
                    {
                        let query = self.search_query.trim();
                        let mut rendered_index = 0;

                        for song in songs {
                            if !query.is_empty() && !crate::utils::song_matches_search(song, query)
                            {
                                continue;
                            }

                            items.push((
                                LibraryListItem::Song(song.id),
                                iced::Rectangle::new(
                                    iced::Point::new(
                                        0.0,
                                        song_y_in_row(&metrics, span.top, rendered_index),
                                    ),
                                    iced::Size::new(row_width, LIST_ROW_HEADER),
                                ),
                            ));
                            rendered_index += 1;
                        }
                    }
                }
            }
        }
        items
    }

    /// Obtiene todos los items actualmente visibles en la vista (respetando colapsos y filtros) sin calcular rectángulos.
    pub fn get_all_list_items(&self) -> Vec<LibraryListItem> {
        let mut items = Vec::new();
        if self.is_list_mode() {
            let (_total_h, v_artists) = self.get_view_structure();
            for va in v_artists {
                items.push(LibraryListItem::Artist(va.artist_name.clone()));
                if !va.is_collapsed {
                    for alb in va.albums {
                        let composite_id = format!("{}|{}", va.artist_name, alb.album_hash);
                        items.push(LibraryListItem::Album(composite_id.clone()));
                        if !self.collapsed_albums.contains(&composite_id) {
                            for (song, _) in alb.songs {
                                items.push(LibraryListItem::Song(song.id));
                            }
                        }
                    }
                }
            }
        } else {
            let albums_to_show = self
                .filtered_albums
                .as_ref()
                .or(self.cached_albums.as_ref());
            if let Some(albums) = albums_to_show {
                for album_entry in albums {
                    let composite_id = format!("{}|{}", album_entry.artist, album_entry.id);
                    items.push(LibraryListItem::Album(composite_id.clone()));
                    if self.expanded_album.as_deref() == Some(composite_id.as_str()) {
                        if let Some(songs) = &self.expanded_album_songs {
                            for song in songs {
                                items.push(LibraryListItem::Song(song.id));
                            }
                        }
                    }
                }
            }
        }
        items
    }

    /// Calcula la tarea de scroll para asegurar que el elemento seleccionado sea visible.
    /// Si 'force_top' es true, el elemento se moverá directamente a la parte superior.
    pub fn get_scroll_task<Message: 'static>(&self, force_top: bool) -> Task<Message> {
        let scroll_id = LIBRARY_SCROLL_ID.clone();
        let margin_top = match self.view_mode {
            LibraryViewMode::DetailedList => LIST_ROW_HEADER,
            LibraryViewMode::ThumbnailList => LIST_ROW,
            LibraryViewMode::Grid => 0.0, // Barra superior + tabs
            _ => LIST_ROW_HEADER,
        };

        // FAST PATH: use the exact hint coordinates stored during keyboard navigation.
        if let Some((hint_y, hint_h)) = self.selected_item_hint {
            let is_artist = matches!(self.focused_item, Some(LibraryListItem::Artist(_)));
            let is_song = matches!(self.focused_item, Some(LibraryListItem::Song(_)));
            let current_margin_top = if self.view_mode == LibraryViewMode::Grid && is_song {
                0.0
            } else {
                margin_top
            };

            // Solo los artistas (sticky headers) tienen margen 0.
            // Los álbumes y canciones deben respetar el margen para no quedar ocultos bajo el sticky header del artista.
            let effective_margin_top = if is_artist { 0.0 } else { current_margin_top };
            let margin_bottom: f32 = 0.0; // Desactivamos el margen dinámico que causaba saltos/espacios vacíos

            if force_top {
                return iced::widget::operation::scroll_to(
                    scroll_id,
                    iced::widget::operation::AbsoluteOffset {
                        x: 0.0,
                        y: (hint_y - effective_margin_top).max(0.0),
                    },
                );
            }

            if let Some(viewport) = &self.last_viewport {
                let view_min = self.scroll_offset.y;
                let view_max = view_min + viewport.height;
                let bottom = hint_y + hint_h;

                if hint_y < view_min + effective_margin_top {
                    return iced::widget::operation::scroll_to(
                        scroll_id,
                        iced::widget::operation::AbsoluteOffset {
                            x: 0.0,
                            y: (hint_y - effective_margin_top).max(0.0),
                        },
                    );
                } else if bottom > view_max - margin_bottom {
                    let offset = (bottom - viewport.height + margin_bottom).max(0.0);
                    return iced::widget::operation::scroll_to(
                        scroll_id,
                        iced::widget::operation::AbsoluteOffset { x: 0.0, y: offset },
                    );
                }
            } else {
                let offset = (hint_y - effective_margin_top).max(0.0);
                return iced::widget::operation::scroll_to(
                    scroll_id,
                    iced::widget::operation::AbsoluteOffset { x: 0.0, y: offset },
                );
            }
            return Task::none();
        }

        // FALLBACK: name-based search (used when clicking with mouse)
        let items = self.get_visible_items();
        for (item, bounds) in &items {
            let is_match = if let Some(focused) = &self.focused_item {
                item == focused
            } else {
                match item {
                    LibraryListItem::Song(id) => {
                        if let Some(idx) = self.selected_song_idx {
                            let list = if self.view_mode == LibraryViewMode::Grid {
                                self.expanded_album_songs.as_ref()
                            } else {
                                self.filtered_songs.as_ref()
                            };
                            list.and_then(|l| l.get(idx))
                                .map(|s| s.id == *id)
                                .unwrap_or(false)
                        } else {
                            false
                        }
                    }
                    LibraryListItem::Album(id) => Some(id) == self.selected_album.as_ref(),
                    LibraryListItem::Artist(name) => Some(name) == self.selected_header.as_ref(),
                }
            };

            if is_match {
                let is_artist = matches!(item, LibraryListItem::Artist(_));
                let is_song = matches!(item, LibraryListItem::Song(_));
                let current_margin_top = if self.view_mode == LibraryViewMode::Grid && is_song {
                    0.0
                } else {
                    margin_top
                };

                let effective_margin_top = if is_artist { 0.0 } else { current_margin_top };
                let margin_bottom: f32 = 0.0;

                let y = bounds.y;
                let h = bounds.height;

                if force_top {
                    return iced::widget::operation::scroll_to(
                        scroll_id,
                        iced::widget::operation::AbsoluteOffset {
                            x: 0.0,
                            y: (y - effective_margin_top).max(0.0),
                        },
                    );
                }

                if let Some(viewport_rect) = &self.last_viewport {
                    let view_min = self.scroll_offset.y;
                    let view_max = view_min + viewport_rect.height;
                    let bottom = y + h;

                    if y < view_min + effective_margin_top {
                        return iced::widget::operation::scroll_to(
                            scroll_id,
                            iced::widget::operation::AbsoluteOffset {
                                x: 0.0,
                                y: (y - effective_margin_top).max(0.0),
                            },
                        );
                    } else if bottom > view_max - margin_bottom {
                        let offset = (bottom - viewport_rect.height + margin_bottom).max(0.0);
                        return iced::widget::operation::scroll_to(
                            scroll_id,
                            iced::widget::operation::AbsoluteOffset { x: 0.0, y: offset },
                        );
                    }
                } else {
                    return iced::widget::operation::scroll_to(
                        scroll_id,
                        iced::widget::operation::AbsoluteOffset {
                            x: 0.0,
                            y: (y - effective_margin_top).max(0.0),
                        },
                    );
                }
                break;
            }
        }
        Task::none()
    }
}
