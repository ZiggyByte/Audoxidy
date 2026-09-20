//! Library virtualization: unloading, cache invalidation, and visible-element math.

use iced::Task;

use super::state::{LIBRARY_SCROLL_ID, LibraryManager};
use super::types::{
    LibraryAlbumGroup, LibraryGridElement, LibraryGridRowData, LibraryListItem, LibraryViewMode,
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
                self.filtered_albums = None;
                cleared = true;
            }
        }

        if let Some(songs) = &mut self.cached_all_songs {
            if songs.len() > 500 {
                tracing::debug!("Audoxidy GC: Cleaning inactive songs from the library");
                songs.truncate(500);
                self.filtered_songs = None;
                cleared = true;
            }
        }

        if cleared {
            self.invalidate_cache();
            self.data_unloaded = true;
        }
    }

    pub fn invalidate_cache(&mut self) {
        self.cached_detailed_view.replace(None);
        self.cached_visible_elements.replace(None);
        // Al invalidar caché por cambio de estructura, el hint de scroll ya no es válido
        self.selected_item_hint = None;
    }

    pub fn invalidate_visible_cache(&mut self) {
        self.cached_visible_elements.replace(None);
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
        if let Some(albums_cache) = &self.cached_albums {
            for alb in albums_cache {
                album_hash_map.insert((alb.artist.clone(), alb.title.clone()), alb.id.clone());
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

                let album_hash = album_hash_map
                    .get(&(group.name.clone(), alb_name.clone()))
                    .cloned()
                    .unwrap_or_else(|| alb_name.clone());

                albums_map.push(LibraryAlbumGroup {
                    album_name: alb_name,
                    album_hash,
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

        let (total_content_h, v_artists) = self.get_view_structure();
        let viewport_h = self
            .last_viewport
            .as_ref()
            .map(|v| v.height)
            .unwrap_or(800.0);
        let view_min_raw = self.scroll_offset.y;

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

        self.cached_visible_elements.replace(Some((
            top_space,
            bottom_space,
            total_content_h,
            visible_elements.clone(),
            sticky_artist_info.clone(),
        )));
        (
            top_space,
            bottom_space,
            total_content_h,
            visible_elements,
            sticky_artist_info,
        )
    }

    /// Calcula los elementos visibles en la cuadrícula para el modo Grid, con virtualización avanzada.
    #[tracing::instrument(skip(self))]
    pub fn get_visible_grid_elements(
        &self,
        columns: usize,
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

        let mut total_height = 0.0;
        let mut rows_data = Vec::with_capacity(albums.len() / columns + 1);

        // Optimización: Desglosar el ID expandido para evitar format! en el bucle
        let (exp_art, exp_alb) = if let Some(exp) = self.expanded_album.as_deref() {
            if let Some(pos) = exp.find('|') {
                (Some(&exp[..pos]), Some(&exp[pos + 1..]))
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };

        // 1. Calcular estructura completa (Altura de cada fila)
        for chunk in albums.chunks(columns) {
            let mut row_h = 252.0;
            let mut expanded_id = None;

            for album_entry in chunk {
                if exp_art == Some(album_entry.artist.as_str())
                    && exp_alb == Some(album_entry.id.as_str())
                {
                    let composite_id = self.expanded_album.clone().unwrap();
                    expanded_id = Some(composite_id);
                    let songs_count = if let Some(songs) = self.expanded_album_songs.as_ref() {
                        let query = self.search_query.trim();
                        if query.is_empty() {
                            songs.len()
                        } else {
                            songs
                                .iter()
                                .filter(|s| crate::utils::song_matches_search(s, query))
                                .count()
                        }
                    } else {
                        1 // "Cargando..."
                    };
                    // 60px de padding/headers + canciones (32px cada una)
                    row_h += 60.0 + (songs_count.max(1) as f32 * LIST_ROW_HEADER);
                    break;
                }
            }
            rows_data.push(LibraryGridRowData {
                top_y: total_height,
                height: row_h,
                albums: chunk.to_vec(),
                expanded_id: expanded_id,
            });
            total_height += row_h;
        }

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

        for row in rows_data {
            let row_end = row.top_y + row.height;
            if row_end < render_min {
                top_space += row.height;
            } else if row.top_y > render_max {
                bottom_space += row.height;
            } else {
                // Fila de álbumes
                visible_elements.push(LibraryGridElement::AlbumRow {
                    albums: row.albums,
                    active_expansion_id: row.expanded_id.clone(),
                });

                // Si esta fila tiene un álbum expandido, virtualizar las canciones dentro del bloque
                if let Some(exp_id) = row.expanded_id {
                    if let Some(songs) = self.expanded_album_songs.as_ref() {
                        let query = self.search_query.trim();

                        // El bloque de canciones empieza después de las tarjetas (252px) + header (32px)
                        let songs_block_start_y = row.top_y + 252.0 + LIST_ROW_HEADER;

                        let mut v_songs = Vec::new();
                        let mut songs_top_space = 0.0;
                        let mut songs_bottom_space = 0.0;

                        let mut current_song_y = songs_block_start_y;
                        for (idx, song) in songs.iter().enumerate() {
                            if !query.is_empty() && !crate::utils::song_matches_search(song, query)
                            {
                                continue;
                            }

                            if current_song_y + LIST_ROW_HEADER >= render_min
                                && current_song_y <= render_max
                            {
                                v_songs.push((song.clone(), idx));
                            } else if current_song_y + LIST_ROW_HEADER < render_min {
                                songs_top_space += LIST_ROW_HEADER;
                            } else {
                                songs_bottom_space += LIST_ROW_HEADER;
                            }
                            current_song_y += LIST_ROW_HEADER;
                        }

                        if !v_songs.is_empty() || songs_top_space > 0.0 || songs_bottom_space > 0.0
                        {
                            visible_elements.push(LibraryGridElement::ExpandedSongs {
                                album_id: exp_id,
                                songs: v_songs,
                                top_space: songs_top_space,
                                bottom_space: songs_bottom_space,
                            });
                        }
                    }
                }
            }
        }

        (top_space, bottom_space, visible_elements)
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
        let mut current_y = 0.0;
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
                let per_row = self.albums_per_row.get().max(1);
                let card_inner_width = 182.0;
                let spacing = 5.0;
                let x_start = 5.0; // El padding left: 5.0 definido en library.rs para content_with_deselection

                for chunk in albums.chunks(per_row) {
                    let mut row_h = 252.0;
                    let mut expanded_id = None;

                    for (col, album_entry) in chunk.iter().enumerate() {
                        let composite_id = format!("{}|{}", album_entry.artist, album_entry.id);
                        let x = x_start + col as f32 * (card_inner_width + spacing);
                        // Usamos el ancho exacto de la tarjeta (182) para una selección precisa
                        items.push((
                            LibraryListItem::Album(composite_id.clone()),
                            iced::Rectangle::new(
                                iced::Point::new(x, current_y),
                                iced::Size::new(card_inner_width, 252.0),
                            ),
                        ));

                        if self.expanded_album.as_deref() == Some(composite_id.as_str()) {
                            expanded_id = Some(composite_id);
                        }
                    }

                    if let Some(_exp_id) = expanded_id {
                        if let Some(songs) = self.expanded_album_songs.as_ref() {
                            let query = self.search_query.trim();
                            let filtered = if query.is_empty() {
                                songs.clone()
                            } else {
                                songs
                                    .iter()
                                    .filter(|s| crate::utils::song_matches_search(s, query))
                                    .cloned()
                                    .collect()
                            };

                            let mut song_y = current_y + 252.0 + LIST_ROW_HEADER; // aprox padding
                            for song in filtered {
                                items.push((
                                    LibraryListItem::Song(song.id),
                                    iced::Rectangle::new(
                                        iced::Point::new(0.0, song_y),
                                        iced::Size::new(lib_width, LIST_ROW_HEADER),
                                    ),
                                ));
                                song_y += LIST_ROW_HEADER;
                            }
                            row_h += 60.0 + (songs.len() as f32 * LIST_ROW_HEADER); // Ajuste de altura de fila para navegación
                        }
                    }
                    current_y += row_h;
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
