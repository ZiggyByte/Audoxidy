use eframe::egui;
use std::sync::Arc;
use crate::library::db::MediaLibrary;
use crate::library::scanner::LibraryScanner;
use crate::audio::AudioManager;
use crate::gui::theme::{COLOR_TEXT_PRIMARY, COLOR_BG};

#[derive(Debug, Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]
enum LibraryView {
    #[default]
    List,
    DetailedList,
    Grid,
}

pub fn show_library(
    ui: &mut egui::Ui,
    audio_manager: &AudioManager,
    library: Arc<MediaLibrary>,
    scanner: &LibraryScanner,
) {
    // Usamos un ID fijo para que el estado persista independientemente del docking
    let view_id = egui::Id::new("library_view_state_global");
    let mut view = ui.data_mut(|d| *d.get_temp_mut_or_default::<LibraryView>(view_id));

    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            ui.heading("Biblioteca Musical");
            
            ui.add_space(20.0);
            
            // Selector de vistas
            ui.selectable_value(&mut view, LibraryView::List, "📄");
            ui.selectable_value(&mut view, LibraryView::DetailedList, "🖼️ Lista");
            ui.selectable_value(&mut view, LibraryView::Grid, "📱 Cuadrícula");

            ui.data_mut(|d| d.insert_temp(view_id, view));

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("📁 Escanear Carpeta").clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        scanner.scan_directory(path);
                    }
                }
            });
        });

        ui.separator();

        // Obtener canciones de la base de datos
        let songs = library.get_all_songs().unwrap_or_default();

        if songs.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);
                ui.label("Tu biblioteca está vacía.");
                ui.label("Haz clic en 'Escanear Carpeta' para añadir música.");
            });
        } else {
            egui::ScrollArea::vertical().show(ui, |ui| {
                match view {
                    LibraryView::List => draw_list_view(ui, audio_manager, &songs),
                    LibraryView::DetailedList => draw_detailed_list_view(ui, audio_manager, &songs),
                    LibraryView::Grid => draw_grid_view(ui, audio_manager, &songs),
                }
            });
        }
    });
}

fn draw_list_view(ui: &mut egui::Ui, audio_manager: &AudioManager, songs: &[crate::library::metadata::LibrarySong]) {
    for song in songs {
        ui.horizontal(|ui| {
            ui.add(egui::Image::new(egui::include_image!("../../assets/icons/library-music-outlined.svg"))
                .tint(COLOR_TEXT_PRIMARY)
                .max_width(14.0));
            
            let label = format!("{} - {}", song.artist, song.title);
            if ui.add(egui::Button::new(label).selected(false).frame(false)).clicked() {
                let _ = audio_manager.load_file(&song.path);
                audio_manager.play();
            }
        });
    }
}

fn draw_detailed_list_view(ui: &mut egui::Ui, audio_manager: &AudioManager, songs: &[crate::library::metadata::LibrarySong]) {
    for song in songs {
        ui.horizontal(|ui| {
            // Miniatura (Placeholder o extracción rápida)
            let thumb_size = 40.0;
            let thumb_rect = ui.allocate_exact_size(egui::vec2(thumb_size, thumb_size), egui::Sense::hover()).0;
            ui.painter().rect_filled(thumb_rect, 4.0, egui::Color32::from_gray(50));
            ui.painter().text(thumb_rect.center(), egui::Align2::CENTER_CENTER, "🎵", egui::FontId::proportional(20.0), COLOR_TEXT_PRIMARY);

            ui.vertical(|ui| {
                ui.label(egui::RichText::new(&song.title).strong());
                ui.label(egui::RichText::new(&song.artist).size(12.0));
            });

            if ui.interact(ui.max_rect(), ui.id().with(&song.path), egui::Sense::click()).clicked() {
                let _ = audio_manager.load_file(&song.path);
                audio_manager.play();
            }
        });
        ui.add_space(5.0);
    }
}

fn draw_grid_view(ui: &mut egui::Ui, audio_manager: &AudioManager, songs: &[crate::library::metadata::LibrarySong]) {
    // Agrupar por álbum
    let mut albums: std::collections::HashMap<String, Vec<&crate::library::metadata::LibrarySong>> = std::collections::HashMap::new();
    for song in songs {
        albums.entry(song.album.clone()).or_default().push(song);
    }

    let item_width = 140.0;
    let spacing = 15.0;
    let available_width = ui.available_width();
    let columns = (available_width / (item_width + spacing)).floor().max(1.0) as usize;
    
    // Estado de expansión
    let expanded_album_key = egui::Id::new("library_expanded_album_global");
    let mut expanded_album = ui.data_mut(|d| d.get_temp::<String>(expanded_album_key));

    let mut album_list: Vec<_> = albums.keys().cloned().collect();
    album_list.sort();

    let rows = (album_list.len() as f32 / columns as f32).ceil() as usize;

    for row in 0..rows {
        ui.horizontal(|ui| {
            for col in 0..columns {
                let idx = row * columns + col;
                if idx >= album_list.len() { break; }
                
                let album_name = &album_list[idx];
                let songs_in_album = &albums[album_name];
                let artist_name = songs_in_album[0].artist.clone();
                let year = songs_in_album[0].year.map(|y| y.to_string()).unwrap_or_default();

                ui.vertical(|ui| {
                    let rect = ui.allocate_exact_size(egui::vec2(item_width, item_width), egui::Sense::click()).0;
                    
                    // Tarjeta de álbum (Capa 2: Portada o Placeholder)
                    ui.painter().rect_filled(rect, 8.0, COLOR_BG.linear_multiply(1.5));
                    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "💿", egui::FontId::proportional(40.0), COLOR_TEXT_PRIMARY);

                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.add(egui::Label::new(egui::RichText::new(album_name).strong()).truncate());
                            ui.add(egui::Label::new(egui::RichText::new(&artist_name).size(11.0).color(egui::Color32::from_gray(150))).truncate());
                            if !year.is_empty() {
                                ui.label(egui::RichText::new(&year).size(10.0).color(egui::Color32::from_gray(100)));
                            }
                        });
                        
                        // Flecha de Despliegue (Chevron)
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let is_expanded = expanded_album.as_ref() == Some(album_name);
                            let icon = if is_expanded { "🔼" } else { "🔽" };
                            if ui.selectable_label(is_expanded, icon).clicked() {
                                if is_expanded {
                                    expanded_album = None;
                                } else {
                                    expanded_album = Some(album_name.clone());
                                }
                                ui.data_mut(|d| d.insert_temp(expanded_album_key, expanded_album.clone()));
                            }
                        });
                    });

                    if ui.interact(rect, ui.id().with(album_name), egui::Sense::click()).clicked() {
                        // Play album logic
                    }
                });
            }
        });

        // Contenido Expandido (Inline Expansion)
        if let Some(expanded_name) = &expanded_album {
            // Verificar si el álbum expandido está en esta fila
            let start_idx = row * columns;
            let end_idx = (row + 1) * columns;
            let current_row_albums = &album_list[start_idx..end_idx.min(album_list.len())];
            
            if current_row_albums.contains(expanded_name) {
                ui.add_space(10.0);
                ui.vertical(|ui| {
                    ui.painter().rect_filled(ui.available_rect_before_wrap(), 4.0, egui::Color32::from_black_alpha(50));
                    ui.indent("tracklist", |ui| {
                        let album_songs = &albums[expanded_name];
                        for song in album_songs {
                            ui.horizontal(|ui| {
                                ui.label(format!("{}.", song.track_number.unwrap_or(0)));
                                if ui.link(&song.title).clicked() {
                                    let _ = audio_manager.load_file(&song.path);
                                    audio_manager.play();
                                }
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.label(format!("{}:{:02}", song.duration_sec / 60, song.duration_sec % 60));
                                });
                            });
                        }
                    });
                });
                ui.add_space(20.0);
            }
        }
    }
}

pub fn show_library_filters(ui: &mut egui::Ui, _library: Arc<MediaLibrary>) {
    ui.vertical(|ui| {
        ui.heading("Filtros");
        ui.separator();
        
        let _ = ui.selectable_label(true, "📋 Todas las canciones");
        let _ = ui.selectable_label(false, "👤 Artistas");
        let _ = ui.selectable_label(false, "💿 Álbumes");
        let _ = ui.selectable_label(false, "🎸 Géneros");
        
        ui.add_space(20.0);
        ui.heading("Carpetas");
        ui.label("📁 Música");
    });
}
