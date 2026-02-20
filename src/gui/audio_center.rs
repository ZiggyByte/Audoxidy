use eframe::egui;
use crate::audio::engine::{AudioSettings, BitDepth, ChannelConfig, AudioDeviceInfo};
use crate::audio::AudioManager;
use crate::audio::EqPreset;
use std::process::Command;

// Estructura principal que mantiene el estado de la ventana
pub struct AudioCenter {
    pub open: bool,
    state: AudioCenterState,
    first_open: bool,
}

impl Default for AudioCenter {
    fn default() -> Self {
        Self {
            open: false,
            state: AudioCenterState::default(),
            first_open: true,
        }
    }
}

// Estructura para estado interno del UI
struct AudioCenterState {
    selected_tab: usize,


    
    // Caché para evitar llamadas continuas a cpal
    cached_hosts: Vec<String>,
    cached_devices: Vec<AudioDeviceInfo>,

    // Estados temporales de selección para "Configuración de Audio"
    selected_host: Option<String>,
    selected_device: Option<String>,
    selected_sample_rate: Option<u32>,
    selected_bit_depth: Option<BitDepth>,
    selected_buffer_size: Option<u32>, // Quantum
    channels_auto: bool,
    selected_channels_manual: u16,
    
    // Equalizer State
    equalizer_enabled: bool,
    equalizer_bands_31: bool,
    preamp_gain: f32,

    // Presets UI
    show_load_preset: bool,
    show_save_preset: bool,
    preset_name_input: String,
    presets: Vec<EqPreset>,
    selected_preset_index: Option<usize>,
    temp_eq_backup: Option<(f32, Vec<f32>)>, // preamp, active bands

    focused_slider: Option<String>,

    apply_enabled: bool, // Para activar botón "Aplicar" solo si hay cambios
}

impl Default for AudioCenterState {
    fn default() -> Self {
        Self {
            selected_tab: 0,


            cached_hosts: Vec::new(),
            cached_devices: Vec::new(),
            selected_host: None,
            selected_device: None,
            selected_sample_rate: None,
            selected_bit_depth: Some(BitDepth::Bits32Float),
            selected_buffer_size: Some(1024),
            channels_auto: false,
            selected_channels_manual: 2,


            equalizer_enabled: false,
            equalizer_bands_31: false,
            preamp_gain: 0.0,
            
            show_load_preset: false,
            show_save_preset: false,
            preset_name_input: String::new(),
            presets: EqPreset::default_presets(),
            selected_preset_index: None,
            temp_eq_backup: None,

            focused_slider: None,

            apply_enabled: false,
        }
    }
}

const TAB_NAMES: [&str; 3] = ["Configuración de Audio", "Ecualizador", "Efectos de Audio"];

impl AudioCenter {
    pub fn show(&mut self, ctx: &egui::Context, audio_manager: &AudioManager) {
        if !self.open { 
            return; 
        }

        // Sync inicial al abrir (y cacheo de dispositivos)
        if self.first_open {
            self.sync_from_engine(audio_manager);
            self.first_open = false;
            self.state.apply_enabled = false;
        }

        let ui_state = &mut self.state;
        let mut is_open = self.open;

        // Definiciones de Color desde Theme
        let color_bg = crate::gui::theme::COLOR_BG; //#000000
        let color_accent = crate::gui::theme::COLOR_ACCENT; // #FF003D
        let color_contrast = crate::gui::theme::COLOR_CONTRAST; // #111111
        let color_text_main = crate::gui::theme::COLOR_TEXT_PRIMARY; // #c6c6c6
        let color_text_sec = crate::gui::theme::COLOR_TEXT_SECONDARY; // #717171

        let window_size = egui::vec2(940.0, 474.0);
        let screen_rect = ctx.input(|i| i.content_rect());
        let pos = screen_rect.center() - window_size / 2.0;

        // Area de bloqueo para modal (si popup activo)
        let interact_enabled = true;

        egui::Window::new("AudioCenter")
            .fixed_size(window_size)
            .fixed_pos(pos)
            .collapsible(false)
            .resizable(false)
            .title_bar(false)
            .frame(egui::Frame::NONE.fill(color_bg).stroke(egui::Stroke::new(2.0, color_accent)))
            .show(ctx, |ui| {
                if ctx.input(|i| i.pointer.any_click()) {
                    ui_state.focused_slider = None;
                }
                
                ui.add_enabled_ui(interact_enabled, |ui| {
                    let rect = ui.max_rect();
                    ui.allocate_rect(rect, egui::Sense::click()); // Consume clicks
                    ui.painter().rect_filled(rect, 0.0, color_bg);

                    // --- Barra de Título (34px) ---
                    let header_rect = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), 34.0));
                    
                    // Título Centrado
                    ui.add_space(5.0);
                    ui.painter().text(
                        header_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "Centro de Audio Avanzado",
                        egui::FontId::proportional(16.0),
                        color_text_main
                    );

                    // Botón Cerrar (Alineado a la derecha)
                    let close_size = 24.0;
                    let close_rect = egui::Rect::from_center_size(
                        egui::pos2(header_rect.max.x - 25.0, header_rect.center().y),
                        egui::vec2(close_size, close_size),
                    );
                    
                    if ui.interact(close_rect, ui.id().with("close_btn"), egui::Sense::click()).clicked() {
                        is_open = false;
                    }
                    ui.painter().text(close_rect.center(), egui::Align2::CENTER_CENTER, "X", egui::FontId::proportional(20.0), color_text_main);

                    // --- Contenido (Padding 15px) ---
                    let content_rect = rect.shrink(15.0);
                    let content_rect = egui::Rect::from_min_max(
                        egui::pos2(content_rect.min.x, header_rect.max.y), 
                        content_rect.max
                    );

                    ui.scope_builder(egui::UiBuilder::new().max_rect(content_rect), |ui| {
                        ui.add_space(5.0);

                        // --- Pestañas ---
                        ui.horizontal(|ui| {
                             ui.spacing_mut().item_spacing = egui::vec2(10.0, 0.0); 
                             for (i, name) in TAB_NAMES.iter().enumerate() {
                                 let is_selected = ui_state.selected_tab == i;
                                 let text_color = if is_selected { egui::Color32::WHITE } else { color_text_sec };
                                 let bg_color = if is_selected { color_accent } else { egui::Color32::TRANSPARENT };

                                 let text_layout = ui.painter().layout_no_wrap(name.to_string(), egui::FontId::proportional(14.0), text_color);
                                 let size = text_layout.size() + egui::vec2(20.0, 15.0); // 20px H padding * 2, 15px V padding * 2
                                 
                                 let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
                                 
                                 if response.clicked() {
                                     ui_state.selected_tab = i;
                                 }
                                 
                                 // Hover
                                 let mut final_text_color = text_color;
                                 if response.hovered() && !is_selected {
                                     final_text_color = color_text_main;
                                 }
                                 
                                 if is_selected {
                                     ui.painter().rect_filled(rect, 0.0, bg_color);
                                 }
                                 
                                 // Centered Text
                                 let text_layout_hover = ui.painter().layout_no_wrap(name.to_string(), egui::FontId::proportional(14.0), final_text_color);
                                 let text_pos = rect.center() - (text_layout_hover.size() / 2.0);
                                 ui.painter().galley(text_pos, text_layout_hover, egui::Color32::TRANSPARENT); // Use color in layout
                             }
                        });

                        ui.add_space(-8.0);

                        // Borde divisor
                        let divider_rect = egui::Rect::from_min_size(
                            ui.cursor().min + egui::vec2(0.0, 5.0),
                            egui::vec2(ui.available_width(), 2.0)
                        );
                        ui.painter().rect_filled(divider_rect, 0.0, color_contrast);
                        ui.add_space(15.0);

                        // --- Contenido de Pestaña ---
                        match ui_state.selected_tab {
                            0 => Self::tab_audio_config(ui, audio_manager, ui_state, color_accent, color_text_main, color_text_sec, color_contrast),
                            1 => Self::tab_equalizer(ui, ui_state, color_accent, color_text_main, color_contrast, audio_manager),
                            2 => Self::tab_audio_effects(ui, ui_state, color_accent, color_text_main, color_contrast, color_text_sec, audio_manager),
                            _ => {}
                        }
                    });
                });
            });
        
        // --- Modals de Presets ---
        if ui_state.show_load_preset {
            let mut close_modal = false;
            let mut revert = false;

            egui::Area::new(egui::Id::new("load_preset_bg"))
                .order(egui::Order::Foreground)
                .fixed_pos(egui::pos2(0.0, 0.0))
                .show(ctx, |ui| {
                    let screen_rect = ctx.input(|i| i.content_rect());
                    let bg_response = ui.allocate_rect(screen_rect, egui::Sense::click());
                    ui.painter().rect_filled(screen_rect, 0.0, egui::Color32::from_black_alpha(150));
                    
                    if bg_response.clicked() {
                        close_modal = true;
                        revert = true;
                    }
                    
                    let popup_size = egui::vec2(310.0, 300.0);
                    let popup_rect = egui::Rect::from_center_size(screen_rect.center(), popup_size);
                    
                    let _popup_interact = ui.allocate_rect(popup_rect, egui::Sense::click());
                    
                    ui.painter().rect_filled(popup_rect, 4.0, color_bg);
                    ui.painter().rect_stroke(popup_rect, 4.0, egui::Stroke::new(2.0, color_accent), egui::StrokeKind::Middle);
                    
                    // Sort presets first
                    let default_names = [
                        "Default", "Pop", "Party", "Electronic", "Rock And Roll", "Rock",
                        "Hard Rock", "Heavy Metal", "Punk Rock", "Jazz", "Soul", "Funk",
                        "Classical", "Ballad", "Mariachi", "Salsa", "Hip-Hop", "Reggaeton"
                    ];
                    
                    ui_state.presets.sort_by(|a, b| {
                        if a.name == "Default" { return std::cmp::Ordering::Less; }
                        if b.name == "Default" { return std::cmp::Ordering::Greater; }
                        
                        let a_is_default = default_names.contains(&a.name.as_str());
                        let b_is_default = default_names.contains(&b.name.as_str());
                        
                        if a_is_default != b_is_default {
                            if a_is_default { std::cmp::Ordering::Greater } else { std::cmp::Ordering::Less }
                        } else {
                            let a_first = a.name.chars().next().unwrap_or('a');
                            let b_first = b.name.chars().next().unwrap_or('a');
                            let a_is_alpha = a_first.is_alphabetic();
                            let b_is_alpha = b_first.is_alphabetic();
                            if a_is_alpha != b_is_alpha {
                                if a_is_alpha { std::cmp::Ordering::Greater } else { std::cmp::Ordering::Less }
                            } else {
                                a.name.to_lowercase().cmp(&b.name.to_lowercase())
                            }
                        }
                    });

                    ui.scope_builder(egui::UiBuilder::new().max_rect(popup_rect.shrink(15.0)), |ui| {
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                            // Columna Izquierda: Lista de Presets
                            ui.allocate_ui(egui::vec2(136.0, 268.0), |ui| {
                                ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                                    egui::Frame::NONE.fill(color_contrast).stroke(egui::Stroke::new(2.0, color_text_main)).inner_margin(5.0).show(ui, |ui| {
                                        egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
                                            let mut last_was_custom = false;
                                            for (idx, preset) in ui_state.presets.iter().enumerate() {
                                                let is_default = default_names.contains(&preset.name.as_str());
                                                if idx > 0 && is_default && last_was_custom {
                                                    ui.add(egui::Separator::default().spacing(10.0));
                                                }
                                                last_was_custom = !is_default && preset.name != "Default";
                                                
                                                let selected = ui_state.selected_preset_index == Some(idx);
                                                let label_color = if selected { color_accent } else { color_text_main };
                                                
                                                if ui.selectable_label(selected, egui::RichText::new(&preset.name).color(label_color)).clicked() {
                                                    ui_state.selected_preset_index = Some(idx);
                                                    // Prev preview
                                                    let is_31 = ui_state.equalizer_bands_31;
                                                    let gains = if is_31 { preset.get_gains_31() } else { preset.get_gains_20() };
                                                    ui_state.preamp_gain = preset.preamp_gain;
                                                    audio_manager.set_preamp_gain(preset.preamp_gain);
                                                    for (i, &gain) in gains.iter().enumerate() {
                                                        if i < (if is_31 { 31 } else { 20 }) {
                                                            audio_manager.set_eq_band_gain(i, gain);
                                                        }
                                                    }
                                                }
                                            }
                                        });
                                    });
                                });
                            });
                            
                            ui.add_space(10.0);
                            
                            // Columna Derecha: Botones
                            ui.allocate_ui(egui::vec2(140.0, 268.0), |ui| {
                                 ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                                      ui.add_space(0.0);
                                      let btn_size = egui::vec2(120.0, 20.0);
                                      if ui.add(egui::Button::new(egui::RichText::new("Aceptar").color(color_text_main).size(14.0)).min_size(btn_size)).clicked() {
                                          close_modal = true;
                                          ui_state.temp_eq_backup = None; // clear backup, mantener preview
                                      }
                                      ui.add_space(8.0);
                                      if ui.add(egui::Button::new(egui::RichText::new("Cancelar").color(color_text_main).size(14.0)).min_size(btn_size)).clicked() {
                                          close_modal = true;
                                          revert = true;
                                      }
                                      ui.add_space(8.0);
                                      if ui.add(egui::Button::new(egui::RichText::new("Eliminar").color(color_text_main).size(14.0)).min_size(btn_size)).clicked() {
                                          if let Some(idx) = ui_state.selected_preset_index {
                                              if idx < ui_state.presets.len() {
                                                  ui_state.presets.remove(idx);
                                                  ui_state.selected_preset_index = None;
                                              }
                                          }
                                      }
                                      ui.add_space(8.0);
                                      if ui.add(egui::Button::new(egui::RichText::new("Importar").color(color_text_main).size(14.0)).min_size(btn_size)).clicked() {
                                          if let Some(path) = rfd::FileDialog::new().add_filter("JSON", &["json"]).pick_file() {
                                                if let Ok(content) = std::fs::read_to_string(path) {
                                                     if let Ok(preset) = serde_json::from_str::<EqPreset>(&content) {
                                                         ui_state.presets.push(preset);
                                                     }
                                                }
                                          }
                                      }
                                      ui.add_space(8.0);
                                      if ui.add(egui::Button::new(egui::RichText::new("Exportar").color(color_text_main).size(14.0)).min_size(btn_size)).clicked() {
                                          if let Some(idx) = ui_state.selected_preset_index {
                                               let preset = &ui_state.presets[idx];
                                               if let Ok(json) = serde_json::to_string_pretty(preset) {
                                                   if let Some(path) = rfd::FileDialog::new().add_filter("JSON", &["json"]).set_file_name(&format!("{}.json", preset.name)).save_file() {
                                                        let _ = std::fs::write(path, json);
                                                   }
                                               }
                                          }
                                      }
                                      ui.add_space(8.0);
                                      if ui.add(egui::Button::new(egui::RichText::new("Predeterminado").color(color_text_main).size(14.0)).min_size(btn_size)).clicked() {
                                          ui_state.presets = EqPreset::default_presets();
                                          ui_state.selected_preset_index = Some(0); // Select first "Default"
                                          let p = &ui_state.presets[0];
                                          let is_31 = ui_state.equalizer_bands_31;
                                          let gains = if is_31 { p.get_gains_31() } else { p.get_gains_20() };
                                          ui_state.preamp_gain = p.preamp_gain;
                                          audio_manager.set_preamp_gain(p.preamp_gain);
                                          for (i, &gain) in gains.iter().enumerate() {
                                              if i < (if is_31 { 31 } else { 20 }) {
                                                  audio_manager.set_eq_band_gain(i, gain);
                                              }
                                          }
                                      }
                                      ui.add_space(8.0);
                                      ui.label(egui::RichText::new("Advertencia: Esta operación restablecerá los presets predeterminados y se eliminarán los presets personalizados previamente guardados.").color(egui::Color32::from_rgb(113, 113, 113)).size(10.0));
                                 });
                            });
                        });
                    });
                });

            if close_modal {
                ui_state.show_load_preset = false;
                if revert {
                    if let Some((preamp, bands)) = ui_state.temp_eq_backup.take() {
                        ui_state.preamp_gain = preamp;
                        audio_manager.set_preamp_gain(preamp);
                        for (i, &gain) in bands.iter().enumerate() {
                            if i < (if ui_state.equalizer_bands_31 { 31 } else { 20 }) {
                                audio_manager.set_eq_band_gain(i, gain);
                            }
                        }
                    }
                }
            }
        }

        if ui_state.show_save_preset {
            let mut close_modal = false;

            egui::Area::new(egui::Id::new("save_preset_bg"))
                .order(egui::Order::Foreground)
                .fixed_pos(egui::pos2(0.0, 0.0))
                .show(ctx, |ui| {
                    let screen_rect = ctx.input(|i| i.content_rect());
                    let bg_response = ui.allocate_rect(screen_rect, egui::Sense::click());
                    ui.painter().rect_filled(screen_rect, 0.0, egui::Color32::from_black_alpha(150));
                    
                    if bg_response.clicked() { close_modal = true; }
                    
                    let popup_size = egui::vec2(180.0, 180.0);
                    let popup_rect = egui::Rect::from_center_size(screen_rect.center(), popup_size);
                    
                    let _popup_interact = ui.allocate_rect(popup_rect, egui::Sense::click());
                    
                    ui.painter().rect_filled(popup_rect, 4.0, color_bg);
                    ui.painter().rect_stroke(popup_rect, 4.0, egui::Stroke::new(2.0, color_accent), egui::StrokeKind::Middle);
                    
                    ui.scope_builder(egui::UiBuilder::new().max_rect(popup_rect.shrink(15.0)), |ui| {
                        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                            ui.label(egui::RichText::new("Guardar Preset").color(color_text_main).size(14.0).strong());
                            ui.add_space(15.0);
                            
                            let name_input = egui::TextEdit::singleline(&mut ui_state.preset_name_input)
                                .text_color(color_text_main)
                                .margin(egui::vec2(5.0, 5.0))
                                .frame(false);
                                
                            egui::Frame::NONE
                                .fill(color_contrast)
                                .stroke(egui::Stroke::new(1.0, color_text_main))
                                .inner_margin(2.0)
                                .show(ui, |ui| {
                                    ui.add(name_input);
                                });
                                
                            ui.add_space(10.0);
                            
                            let save_disabled = ui_state.preset_name_input.trim().is_empty();
                            let exists = ui_state.presets.iter().any(|p| p.name == ui_state.preset_name_input.trim());
                            
                            if exists && !save_disabled {
                                ui.label(egui::RichText::new("Ya existe un preset con ese nombre. ¿Quieres sobrescribirlo?").color(color_accent).size(10.0));
                            } else {
                                ui.add_space(10.0); // Maintain height
                            }
                            
                            ui.add_space(10.0);
                            
                            ui.horizontal(|ui| {
                                ui.add_enabled_ui(!save_disabled, |ui| {
                                    if ui.button(egui::RichText::new("Guardar").color(color_accent)).clicked() {
                                        let preamp = ui_state.preamp_gain;
                                        let is_31 = ui_state.equalizer_bands_31;
                                        let bands_count = if is_31 { 31 } else { 20 };
                                        let mut active_gains = Vec::with_capacity(bands_count);
                                        for i in 0..bands_count {
                                            active_gains.push(audio_manager.get_eq_band_info(i).unwrap_or((0.0, 0.0)).1);
                                        }
                                        
                                        let new_preset = if is_31 {
                                            EqPreset::new(ui_state.preset_name_input.trim(), preamp, None, Some(active_gains))
                                        } else {
                                            EqPreset::new(ui_state.preset_name_input.trim(), preamp, Some(active_gains), None)
                                        };
                                        
                                        if let Some(existing) = ui_state.presets.iter_mut().find(|p| p.name == new_preset.name) {
                                            *existing = new_preset;
                                        } else {
                                            ui_state.presets.push(new_preset);
                                        }
                                        close_modal = true;
                                    }
                                });
                                
                                ui.add_space(10.0);
                                
                                if ui.button(egui::RichText::new("Cancelar").color(color_text_main)).clicked() {
                                    close_modal = true;
                                }
                            });
                        });
                    });
                });

            if close_modal {
                ui_state.show_save_preset = false;
            }
        }

        self.open = is_open;
    }

    fn sync_from_engine(&mut self, manager: &AudioManager) {
        let s = manager.state().read().clone();
        
        // Cargar Cachés
        self.state.cached_hosts = manager.get_available_hosts();
        self.state.cached_devices = manager.get_devices();

        if self.state.selected_host.is_none() {
            // Default: ALSA (if available)
            // "En el servidor de audio: la opcion por defecto debe ser alsa"
            if self.state.cached_hosts.contains(&"ALSA".to_string()) {
                self.state.selected_host = Some("ALSA".to_string());
            } else if let Some(first) = self.state.cached_hosts.first() {
                self.state.selected_host = Some(first.clone());
            }
        }
        
        // Default Device: PipeWire or PulseAudio
        if self.state.selected_device.is_none() {
             // Buscar pipewire o pulse
             let device_list = &self.state.cached_devices;
             let target = device_list.iter().find(|d| d.name.to_lowercase().contains("pipewire"))
                 .or_else(|| device_list.iter().find(|d| d.name.to_lowercase().contains("pulse")))
                 .or_else(|| device_list.iter().find(|d| d.name.to_lowercase().contains("default")));
             
             if let Some(d) = target {
                 self.state.selected_device = Some(d.name.clone());
             } else if let Some(first) = device_list.first() {
                 self.state.selected_device = Some(first.name.clone());
             }
        }

        // Sync values
        // Default Rate: 44100 (if None/Auto in engine? Engine reports current rate)
        // User wants default *selection* to be 44100.
        // Default Rate: 48000 Hz if none
        if self.state.selected_sample_rate.is_none() {
             self.state.selected_sample_rate = Some(48000);
        }
        
        // Channels logic
        match s.config_channels {
            ChannelConfig::Manual(c) => self.state.selected_channels_manual = c,
            ChannelConfig::Auto => self.state.selected_channels_manual = 2, // Default 2.0
        }
        self.state.channels_auto = false;

        // Auto Quantum (None) if set to 0, otherwise value.
        // User wants default Auto. If s.buffer_size is 0, it means auto?
        // Let's assume on first open for user, if they haven't set it, we want Auto (None in dropdown).
        if self.first_open && s.buffer_size == 0 {
             self.state.selected_buffer_size = None;
        } else if s.buffer_size > 0 {
             self.state.selected_buffer_size = Some(s.buffer_size);
        } else {
             self.state.selected_buffer_size = None;
        }
        
        // Bit Depth Default: 32-bit Float
        if self.state.selected_bit_depth.is_none() {
            self.state.selected_bit_depth = Some(BitDepth::Bits32Float);
        }
    }

    fn tab_audio_config(
        ui: &mut egui::Ui, 
        audio_manager: &AudioManager,
        ui_state: &mut AudioCenterState,
        color_accent: egui::Color32,
        color_main: egui::Color32,
        color_sec: egui::Color32,
        color_contrast: egui::Color32
    ) {

        let available_width = ui.available_width();
        let left_width = available_width * 0.50;
        let right_width = available_width - left_width - 0.0; // Spacing logic

        ui.horizontal(|ui| {
             // --- Columna Izquierda (Ajustes) - 60% ---
             ui.allocate_ui_with_layout(
                 egui::vec2(left_width, ui.available_height()), 
                 egui::Layout::top_down(egui::Align::Min), 
                 |ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(0.0, 15.0);

                    // Helper para filas alineadas
                    // Label Izquierda, Combo Derecha (pero pegado/alineado)
                    // Diseño solicitado: "Lista desplegable al lado del nombre... alineado a la izquierda"
                    // "dejando solo un espacio a la derecha" -> Label [Espacio] Combo
                    fn setting_row_aligned(ui: &mut egui::Ui, label: &str, color: egui::Color32, content: impl FnOnce(&mut egui::Ui)) -> bool {
                         let changed = false;
                         ui.horizontal(|ui| {
                            // Label ancho fijo para alineación? o fluido?
                            // "alineados todos a la izquierda".
                            // Usaremos un ancho fijo para las labels para que los combos arranquen en la misma X
                            let label_width = 166.0;
                            ui.allocate_ui(egui::vec2(label_width, 10.0), |ui| {
                                ui.label(egui::RichText::new(label).color(color).size(14.0).strong());
                            });
                            ui.add_space(10.0);
                            content(ui);
                         });
                         changed
                    }

                    // Servidor de Audio
                    ui.add_space(6.0);
                    ui.add_enabled_ui(true, |ui| {

                         setting_row_aligned(ui, "Servidor de Audio:", color_main, |ui| {
                            let mut selected = ui_state.selected_host.as_deref().unwrap_or("Default").to_string();
                            let prev = selected.clone();
                            
                            egui::ComboBox::from_id_salt("host_combo")
                                .width(126.0)
                                .selected_text(&selected)
                                .show_ui(ui, |ui| {
                                    for host in &ui_state.cached_hosts {
                                        ui.selectable_value(&mut selected, host.clone(), host);
                                    }
                                });
                            
                            if selected != prev {
                                ui_state.selected_host = Some(selected);
                                // Trigger device refresh
                                ui_state.apply_enabled = true;
                                // Need to trigger refresh of devices immediately?
                                // Only logic inside engine updates devices.
                                // We need to re-fetch devices for this host.
                                // NOTE: Engine `get_devices` uses current host in Engine, not selected in UI.
                                // This assumes we applied settings.
                                // UI logic flow: Select Host -> Apply -> Validates available devices.
                                // User flow expectation: Select Host -> Devices update?
                                // Complex without async apply. Keep simple: Select Host -> Apply -> Devices Update in list.
                                // Or we can optimize later.
                            }
                         });
                    });

                    // Dispositivo de Salida
                    setting_row_aligned(ui, "Dispositivo de Salida:", color_main, |ui| {
                        let mut selected = ui_state.selected_device.clone().unwrap_or_default();
                        let prev = selected.clone();
                        
                        egui::ComboBox::from_id_salt("device_combo")
                            .width(232.0)
                            .selected_text(if selected.len() > 25 { format!("{}...", &selected[..22]) } else { selected.clone() })
                            .show_ui(ui, |ui| {
                                for device in &ui_state.cached_devices {
                                    ui.selectable_value(&mut selected, device.name.clone(), &device.name);
                                }
                            });
                        
                        if selected != prev {
                            ui_state.selected_device = Some(selected);
                            ui_state.apply_enabled = true;
                        }
                    });

                    // Frecuencia de Muestreo
                    ui.add_enabled_ui(true, |ui| {

                        setting_row_aligned(ui, "Frecuencia de Muestreo:", color_main, |ui| {
                            let mut selected = ui_state.selected_sample_rate;
                            let prev = selected;

                            egui::ComboBox::from_id_salt("rate_combo")
                                .width(126.0)
                                .height(220.0)
                                .selected_text(selected.map(|r| format!("{} Hz", r)).unwrap_or("48000 Hz".into()))
                                .show_ui(ui, |ui| {
                                    // Removed Auto as requested
                                    let rates = [44100, 48000, 88200, 96000, 176400, 192000, 352800, 384000];
                                    for r in rates { ui.selectable_value(&mut selected, Some(r), format!("{} Hz", r)); }
                                });
                            
                            if selected != prev {
                                ui_state.selected_sample_rate = selected;
                                ui_state.apply_enabled = true;
                            }
                        });

                        // Profundidad de Bits
                        setting_row_aligned(ui, "Profundidad de Bits:", color_main, |ui| {
                            let mut selected = ui_state.selected_bit_depth;
                            let prev = selected;

                            egui::ComboBox::from_id_salt("bit_depth_combo")
                                .width(126.0)
                                .selected_text(match selected {
                                    Some(BitDepth::Bits16) => "16-bit Int",
                                    Some(BitDepth::Bits24) => "24-bit Int",
                                    Some(BitDepth::Bits32Float) => "32-bit Float",
                                    None => "32-bit Float"
                                })
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut selected, Some(BitDepth::Bits16), "16-bit Int");
                                    ui.selectable_value(&mut selected, Some(BitDepth::Bits24), "24-bit Int");
                                    ui.selectable_value(&mut selected, Some(BitDepth::Bits32Float), "32-bit Float");
                                });

                             if selected != prev {
                                ui_state.selected_bit_depth = selected;
                                ui_state.apply_enabled = true;
                            }
                        });
                    
                        // Canales de Salida
                        setting_row_aligned(ui, "Canales de Salida:", color_main, |ui| {
                            let mut manual = ui_state.selected_channels_manual;
                            let prev_manual = manual;
                            
                            egui::ComboBox::from_id_salt("channels_combo")
                                .width(126.0)
                                .selected_text(match manual {
                                    1 => "1.0 Mono", 2 => "2.0 Stereo", 3 => "2.1 Stereo + Sub", 4 => "4.0 Quad", 6 => "5.1 Surround", 8 => "7.1 Surround", _ => "Custom"
                                })
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut manual, 1, "1.0 Mono");
                                    ui.selectable_value(&mut manual, 2, "2.0 Stereo");
                                    ui.selectable_value(&mut manual, 3, "2.1 Stereo + Sub");
                                    ui.selectable_value(&mut manual, 4, "4.0 Quad");
                                    ui.selectable_value(&mut manual, 6, "5.1 Surround");
                                    ui.selectable_value(&mut manual, 8, "7.1 Surround");
                                });
                             
                             if manual != prev_manual {
                                 ui_state.selected_channels_manual = manual;
                                 ui_state.apply_enabled = true;
                             }
                             // Auto switch removed
                        });

                        // Quantum / Buffer Size
                        setting_row_aligned(ui, "Quantum (Buffer):", color_main, |ui| {
                            let mut selected = ui_state.selected_buffer_size;
                            let prev = selected;
                            
                            egui::ComboBox::from_id_salt("buffer_combo")
                                .width(126.0)
                                .selected_text(selected.map(|b| format!("{}", b)).unwrap_or("Automático".into()))
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut selected, None, "Automático");
                                    let sizes = [256, 512, 1024, 2048, 4096, 8192];
                                    for s in sizes { ui.selectable_value(&mut selected, Some(s), format!("{}", s)); }
                                });
                            
                            if selected != prev {
                                ui_state.selected_buffer_size = selected;
                                ui_state.apply_enabled = true;
                            }
                        });
                    });

                    ui.add_space(5.0);

                    });

             ui.add_space(15.0);
             let splitter_rect = egui::Rect::from_min_size(ui.cursor().min, egui::vec2(1.0, ui.available_height()));
             ui.painter().rect_filled(splitter_rect, 0.0, color_contrast);
             ui.add_space(15.0);

             // --- Columna Derecha: Información (40%) ---
             // right_width calc above uses 0.60 left.
             ui.allocate_ui_with_layout(
                 egui::vec2(right_width, ui.available_height()),
                 egui::Layout::top_down(egui::Align::Min),
                 |ui| {
                    ui.add_space(6.0);
                    ui.heading(egui::RichText::new("Estado del Audio").color(color_main).size(14.0));
                    ui.add_space(10.0);
                    
                    let state_arc = audio_manager.state();
                    let state_read = state_arc.read();
                    
                    let sample_rate_str = format!("{} Hz", state_read.sample_rate);
                    let channels_str = format!("{}", state_read.channels);
                    let bit_depth_str = state_read.bit_depth_display.clone();
                    let buffer_str = if state_read.buffer_size > 0 { format!("{}", state_read.buffer_size) } else { "Auto".to_string() };

                    // Servidor detected logic placeholder
                    // Can we know it? Engine returns defaults manually.
                    // Servidor detected logic placeholder
                    // Can we know it? Engine returns defaults manually.
                    let backend_str = "Compartido";


                    let info_rows = [
                        ("Backend:", backend_str), 
                        ("Dispositivo:", "Sistema"), 
 
                        ("Frecuencia:", sample_rate_str.as_str()),
                        ("Profundidad:", bit_depth_str.as_str()),
                        ("Canales:", channels_str.as_str()),
                        ("Quantum:", buffer_str.as_str()), 
                    ];

                    for (label, val) in info_rows {
                         ui.horizontal(|ui| {
                             ui.label(egui::RichText::new(label).color(color_sec));
                             ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                 ui.label(egui::RichText::new(val).color(color_main));
                             });
                         });
                         ui.add_space(5.0);
                         ui.separator();
                         ui.add_space(5.0);
                    }
                 }
             );
        });

        // --- Footer Buttons (Bottom Left) ---
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
             ui.add_space(10.0);
             
             ui.horizontal(|ui| {
                 ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                     // Estilo personalizado para botones
                     let mut btn_style = (**ui.style()).clone();
                     btn_style.spacing.button_padding = egui::vec2(10.0, 5.0);
                     btn_style.visuals.widgets.hovered.weak_bg_fill = color_accent;
                     btn_style.visuals.widgets.hovered.bg_fill = color_accent;
                     btn_style.visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, color_main); 
                     
                     ui.scope(|ui| {
                         ui.set_style(btn_style);
                         
                         if ui.add(egui::Button::new(egui::RichText::new("Reiniciar servicio de Audio").color(color_main)).min_size(egui::vec2(0.0, 30.0))).clicked() {
                              restart_audio_service();
                         }
                         
                         ui.add_space(10.0);
                         
                         if ui.add(egui::Button::new(egui::RichText::new("Predeterminado").color(color_main)).min_size(egui::vec2(0.0, 30.0))).clicked() {
                             // Reset UI state to new defaults
                             ui_state.selected_sample_rate = Some(48000); // 48kHz
                             ui_state.selected_bit_depth = Some(BitDepth::Bits32Float); // 32 Float
                             ui_state.selected_buffer_size = None; // Auto Quantum
                             ui_state.channels_auto = false;
                             ui_state.selected_channels_manual = 2; // 2.0 Stereo

                             // Server/Device reset
                             if ui_state.cached_hosts.contains(&"ALSA".to_string()) {
                                ui_state.selected_host = Some("ALSA".to_string());
                             } else if let Some(first) = ui_state.cached_hosts.first() {
                                 ui_state.selected_host = Some(first.clone());
                             }
                             
                             let device_list = &ui_state.cached_devices;
                             // Try Pipewire -> Pulse -> Default
                             let target = device_list.iter().find(|d| d.name.to_lowercase().contains("pipewire"))
                                 .or_else(|| device_list.iter().find(|d| d.name.to_lowercase().contains("pulse")))
                                 .or_else(|| device_list.iter().find(|d| d.name.to_lowercase().contains("default")));
                             
                             if let Some(d) = target { ui_state.selected_device = Some(d.name.clone()); }
                             else if let Some(first) = device_list.first() { ui_state.selected_device = Some(first.name.clone()); }

                             ui_state.apply_enabled = true;
                         }

                         ui.add_space(10.0);

                         // Boton Aplicar (Desactivado si no cambios)
                         ui.add_enabled_ui(ui_state.apply_enabled, |ui| {
                             if ui.add(egui::Button::new(egui::RichText::new("Aplicar").color(color_main)).min_size(egui::vec2(0.0, 30.0))).clicked() {
                                  let settings = AudioSettings {
                                      host_id: ui_state.selected_host.clone(),
                                      device_name: ui_state.selected_device.clone(),
                                      sample_rate: ui_state.selected_sample_rate,
                                      bit_depth: ui_state.selected_bit_depth,
                                      channels: if ui_state.channels_auto { ChannelConfig::Auto } else { ChannelConfig::Manual(ui_state.selected_channels_manual) },
                                      buffer_size: ui_state.selected_buffer_size, 
                                  };
                                  let _ = audio_manager.apply_audio_settings(settings);
                                  ui_state.apply_enabled = false;
                             }
                         });
                     });
                 });
             });
        });
    }

    fn tab_equalizer(ui: &mut egui::Ui, ui_state: &mut AudioCenterState, color_accent: egui::Color32, color_main: egui::Color32, color_contrast: egui::Color32, audio_manager: &AudioManager) {
        ui.add_space(10.0);

        // --- Fila Superior: Toggle, Bandas, Presets, Default ---
        ui.horizontal(|ui| {
            // 1. Toggle Switch Funcional
            let switch_height = 17.0;
            let switch_width = 34.0;
            
            ui.label(egui::RichText::new("Activar Ecualizador:").color(color_main).size(14.0));
            ui.add_space(3.0);

            let (rect, response) = ui.allocate_exact_size(egui::vec2(switch_width, switch_height), egui::Sense::click());
            if response.clicked() {
                ui_state.equalizer_enabled = !ui_state.equalizer_enabled;
                audio_manager.set_eq_enabled(ui_state.equalizer_enabled); 
            }

            // Draw Toggle
            let how_on = ui.ctx().animate_bool(response.id, ui_state.equalizer_enabled);
            let visual_rect = rect;
            let corner_radius = visual_rect.height() / 2.0; // Track corner radius
            
            // Track Color Interpolation
            let track_color_off = egui::Rgba::from(egui::Color32::from_gray(50));
            let track_color_on = egui::Rgba::from(color_accent);
            let track_color: egui::Color32 = (track_color_off * (1.0 - how_on) + track_color_on * how_on).into();

            ui.painter().rect_filled(visual_rect, corner_radius, track_color);
            // Knob
            let knob_radius = visual_rect.height() / 2.0 - 2.0;
            let knob_center = egui::pos2(
                visual_rect.min.x + knob_radius + 2.0 + (visual_rect.width() - 2.0 * knob_radius - 4.0) * how_on,
                visual_rect.center().y,
            );
            ui.painter().circle_filled(knob_center, knob_radius, egui::Color32::WHITE);


            // Espaciador flexible
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                let available = ui.available_width();
                
                // Centrado aproximado de "Bandas"
                let center_content_width = 130.0; 
                let right_content_width = 200.0; 
                let space_left = (available - center_content_width - right_content_width) / 2.0;
                
                ui.add_space(space_left.max(10.0));

                // 2. Bandas
                ui.label(egui::RichText::new("Bandas:").color(color_main).size(14.0));
                ui.add_space(3.0);
                
                // Radio Buttons Logic (Custom Look)
                let bands_opts = [(false, "20"), (true, "31")];
                for (is_31, label) in bands_opts {
                     let selected = ui_state.equalizer_bands_31 == is_31;
                     let icon = if selected { egui::include_image!("../../assets/icons/radio-button-on.svg") } else { egui::include_image!("../../assets/icons/radio-button-off.svg") };
                     let tint = if selected { color_accent } else { color_main };
                     
                     if ui.add(egui::Button::image(egui::Image::new(icon).max_height(26.0).tint(tint)).frame(false)).clicked() {
                         ui_state.equalizer_bands_31 = is_31;
                         // Set EQ mode on logic change
                         audio_manager.set_eq_mode(if is_31 { 31 } else { 20 });
                     }
                     ui.label(egui::RichText::new(label).color(color_main));
                     ui.add_space(5.0);
                }

                // Derecha: Presets & Default
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.menu_button(egui::RichText::new("Presets").color(color_main).size(10.0).strong(), |ui| {
                          if ui.button(egui::RichText::new("Cargar").color(color_main)).clicked() { 
                              ui_state.show_load_preset = true;
                              ui_state.selected_preset_index = None;
                              let count = if ui_state.equalizer_bands_31 { 31 } else { 20 };
                              let mut bands = Vec::with_capacity(count);
                              for i in 0..count {
                                  bands.push(audio_manager.get_eq_band_info(i).unwrap_or((0.0, 0.0)).1);
                              }
                              ui_state.temp_eq_backup = Some((ui_state.preamp_gain, bands));
                              ui.close(); 
                          }
                          if ui.button(egui::RichText::new("Guardar").color(color_main)).clicked() { 
                              ui_state.show_save_preset = true;
                              ui_state.preset_name_input.clear();
                              ui.close(); 
                          }
                    });

                    ui.add_space(7.0);

                    // 3. Predeterminado
                    if ui.button(egui::RichText::new("Predeterminado").color(color_main)).clicked() {
                         ui_state.preamp_gain = 0.0;
                         audio_manager.reset_dsp_defaults();
                         ui_state.apply_enabled = true; // Refresh UI
                    }
                });
            });
        });

        ui.add_space(25.0);

        // --- Contenido Principal (3 Columnas) ---
        // Layout: Legend at bottom, Content above
        
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
             // 1. Legend
             ui.add_space(5.0);
             ui.label(egui::RichText::new("*Puede restablecer al valor por defecto haciendo clic derecho sobre un deslizador.").color(egui::Color32::from_rgb(113, 113, 113)).size(10.0));
             ui.add_space(5.0);

             // 2. Main Content
             ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                  let available_height = ui.available_height();
                  
                  let header_height = 0.0;
                  let slider_height = 240.0;
                  
                  // Col 1: Pre-Amplifier
                  let col1_width = 20.0; 
                  ui.add_space(-2.0); // Padding left
                  ui.allocate_ui(egui::vec2(col1_width, available_height), |ui| {
                        ui.vertical_centered(|ui| {
                            // Fixed Header
                            let (rect, _) = ui.allocate_exact_size(egui::vec2(col1_width, header_height), egui::Sense::hover());
                            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "Pre", egui::FontId::proportional(10.0), color_main);
                            
                            ui.add_space(13.0);
                            
                            // Custom Vertical Slider
                            let slider_size = egui::vec2(14.0, slider_height);
                            let (rect, _) = ui.allocate_exact_size(slider_size, egui::Sense::hover());
                            let mut response = ui.interact(rect, ui.id().with("preamp_slider_id"), egui::Sense::click_and_drag());
                            
                            // Input Handling
                            let is_focused = ui_state.focused_slider.as_deref() == Some("preamp");
                            if response.clicked() || response.dragged() {
                                ui_state.focused_slider = Some("preamp".to_string());
                            }
                            if response.clicked_by(egui::PointerButton::Secondary) {
                                ui_state.preamp_gain = 0.0;
                                if ui_state.equalizer_enabled { audio_manager.set_preamp_gain(0.0); }
                                response.mark_changed();
                            } else if response.dragged() || response.clicked() {
                                 if let Some(pos) = response.interact_pointer_pos() {
                                     let t = 1.0 - (pos.y - rect.min.y) / rect.height();
                                     let t_clamped = t.clamp(0.0, 1.0);
                                     let val = -9.0 + t_clamped * 18.0;
                                     let val_snapped = (val * 10.0).round() / 10.0;
                                     
                                     if (ui_state.preamp_gain - val_snapped).abs() > 0.001 {
                                         ui_state.preamp_gain = val_snapped;
                                         if ui_state.equalizer_enabled {
                                            audio_manager.set_preamp_gain(val_snapped);
                                         }
                                         response.mark_changed();
                                     }
                                 }
                            }
                            
                            // Keyboard handling
                            if is_focused {
                                let mut step = 0.0;
                                ui.input_mut(|i| {
                                    if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) { step = 0.1; }
                                    else if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) { step = -0.1; }
                                });
                                if step != 0.0 {
                                    let val_snapped = (ui_state.preamp_gain + step).clamp(-9.0, 9.0);
                                    let val_snapped = (val_snapped * 10.0).round() / 10.0;
                                    if (ui_state.preamp_gain - val_snapped).abs() > 0.001 {
                                         ui_state.preamp_gain = val_snapped;
                                         if ui_state.equalizer_enabled { audio_manager.set_preamp_gain(val_snapped); }
                                         response.mark_changed();
                                    }
                                }
                            }
                            
                            // Drawing
                            let track_width = 8.0;
                            let track_rect = egui::Rect::from_center_size(rect.center(), egui::vec2(track_width, rect.height()));
                            ui.painter().rect_filled(track_rect, 2.0, color_contrast);
                            
                            // Handle
                            let curr_t = (ui_state.preamp_gain - (-9.0)) / 18.0; 
                            let handle_y = rect.max.y - curr_t * rect.height();
                            let handle_size = egui::vec2(14.0, 16.0);
                            let handle_rect = egui::Rect::from_center_size(egui::pos2(rect.center().x, handle_y), handle_size);
                            
                            let handle_color = if ui_state.equalizer_enabled { color_accent } else { color_contrast };
                            ui.painter().rect_filled(handle_rect, 2.0, handle_color);
                            if is_focused {
                                ui.painter().rect_stroke(handle_rect, 2.0, egui::Stroke::new(1.0, color_main), egui::StrokeKind::Middle);
                            }
                            
                            // Tooltip Zero-Latency
                            if response.hovered() || response.dragged() || is_focused {
                                 let val_display = Self::format_value(ui_state.preamp_gain);
                                 let tooltip_pos = egui::pos2(handle_rect.right() + 8.0, handle_rect.center().y);
                                 let painter = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Tooltip, response.id));
                                 let galley = ui.painter().layout_no_wrap(val_display, egui::FontId::proportional(11.0), color_main);
                                 let text_rect = egui::Rect::from_min_size(tooltip_pos - egui::vec2(0.0, galley.size().y / 2.0), galley.size());
                                 let bg_rect = text_rect.expand(4.0);
                                 painter.rect_filled(bg_rect, 2.0, color_contrast);
                                 painter.rect_stroke(bg_rect, 2.0, egui::Stroke::new(1.0, color_accent), egui::StrokeKind::Middle);
                                 painter.galley(text_rect.min, galley, color_main);
                            }
                            
                            ui.add_space(5.0);
                            
                            // Value Display
                            let display_str = Self::format_value(ui_state.preamp_gain);
                            ui.label(egui::RichText::new(display_str).color(color_main).size(10.0));
                        });
                  });

                  // Col 2: Guía de Niveles (Aligned)
                  let guide_height = ui.available_height(); 
                  ui.allocate_ui(egui::vec2(2.0, guide_height), |ui| {
                        // Fixed Header Space
                        ui.allocate_exact_size(egui::vec2(15.0, header_height), egui::Sense::hover());
                        ui.add_space(-35.5);
                        
                        let guide_draw_height = slider_height; 
                        let (rect, _) = ui.allocate_exact_size(egui::vec2(25.0, guide_draw_height), egui::Sense::hover());
                        let text_col = egui::Color32::from_rgb(113, 113, 113); 

                        // Align text vertically to slider min/center/max
                        // Note: Slider min Y is Top (+9dB), max Y is Bottom (-9dB).
                        // Text default alignment is center of galley.
                        // We want center of text to align with y.

                        let draw_text = |y: f32, txt: &str| {
                           ui.painter().text(egui::pos2(rect.center().x, y), egui::Align2::CENTER_CENTER, txt, egui::FontId::proportional(10.0), text_col);
                        };

                        let offset_top = 22.0;
                        let offset_mid = 15.0;
                        let offset_bot = 9.0;
                        draw_text(rect.min.y + offset_top, "+9");
                        draw_text(rect.center().y + offset_mid, "0");
                        draw_text(rect.max.y + offset_bot, "-9");
                  });

                  // Col 3: Bandas
                  ui.add_space(0.0); // Fix vertical alignment
                  let available_height = ui.available_height();
                  ui.allocate_ui(egui::vec2(ui.available_width(), available_height), |ui| {
                       if !ui_state.equalizer_bands_31 {
                           ui.horizontal(|ui| {
                               ui.add_space(-4.0); // Padding left
                               
                               let band_count = audio_manager.get_eq_bands_count();
                               // Sync backend if needed (e.g. initial load)
                               if band_count != 20 {
                                   audio_manager.set_eq_mode(20);
                               }

                               for i in 0..20 {
                                   let info = audio_manager.get_eq_band_info(i).unwrap_or((0.0, 0.0));
                                   let freq = info.0;
                                   let mut gain = info.1;
                                   
                                   ui.vertical(|ui| {
                                       // Fixed Header for Label
                                       let (rect_header, _) = ui.allocate_exact_size(egui::vec2(18.0, header_height), egui::Sense::hover());
                                       
                                       // Label Logic: Alternating Top/Bottom
                                       let show_top = i % 2 == 0;
                                       let freq_label = if freq >= 1000.0 { format!("{:.1}", freq/1000.0).replace(".0", "") } else { format!("{:.0}", freq) };
                                       
                                       if show_top {
                                            ui.painter().text(rect_header.center(), egui::Align2::CENTER_CENTER, freq_label.clone(), egui::FontId::proportional(10.0), color_main);
                                       }
                                       ui.add_space(13.0);

                                       // Custom Vertical Slider
                                       let slider_size = egui::vec2(14.0, slider_height);
                                       let (rect, _) = ui.allocate_exact_size(slider_size, egui::Sense::hover());
                                       let mut response = ui.interact(rect, ui.id().with("eq_band_20_slider_id").with(i), egui::Sense::click_and_drag());
                                       
                                       // Interaction
                                       let slider_id = format!("eq_20_{}", i);
                                       let is_focused = ui_state.focused_slider.as_deref() == Some(&slider_id);
                                       if response.clicked() || response.dragged() {
                                           ui_state.focused_slider = Some(slider_id);
                                       }
                                       if response.clicked_by(egui::PointerButton::Secondary) {
                                           gain = 0.0;
                                           audio_manager.set_eq_band_gain(i, gain);
                                           response.mark_changed();
                                       } else if response.dragged() || response.clicked() {
                                           if let Some(pos) = response.interact_pointer_pos() {
                                               let t = 1.0 - (pos.y - rect.min.y) / rect.height();
                                               // Range +/- 9dB
                                               let val = -9.0 + t.clamp(0.0, 1.0) * 18.0; 
                                               // Snap to 0.1
                                               let val_snapped = (val * 10.0).round() / 10.0;
                                               
                                               if (gain - val_snapped).abs() > 0.001 {
                                                    gain = val_snapped;
                                                    audio_manager.set_eq_band_gain(i, gain);
                                                    response.mark_changed();
                                               }
                                           }
                                       }
                                       
                                       // Keyboard
                                       if is_focused {
                                           let mut step = 0.0;
                                           ui.input_mut(|inp| {
                                               if inp.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) { step = 0.1; }
                                               else if inp.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) { step = -0.1; }
                                           });
                                           if step != 0.0 {
                                               let val_snapped = (gain + step).clamp(-9.0, 9.0);
                                               let val_snapped = (val_snapped * 10.0).round() / 10.0;
                                               if (gain - val_snapped).abs() > 0.001 {
                                                    gain = val_snapped;
                                                    audio_manager.set_eq_band_gain(i, gain);
                                                    response.mark_changed();
                                               }
                                           }
                                       }
                                       
                                       // Draw Track
                                       let track_width = 8.0;
                                       let track_rect = egui::Rect::from_center_size(rect.center(), egui::vec2(track_width, rect.height()));
                                       ui.painter().rect_filled(track_rect, 2.0, color_contrast);
                                       
                                       // Draw Handle
                                       // Range -9 to +9
                                       let curr_t = (gain - (-9.0)) / 18.0;
                                       let handle_y = rect.max.y - curr_t * rect.height();
                                       let handle_size = egui::vec2(14.0, 16.0);
                                       let handle_rect = egui::Rect::from_center_size(egui::pos2(rect.center().x, handle_y), handle_size);
                                       
                                       let handle_color = if ui_state.equalizer_enabled { color_accent } else { color_contrast };
                                       ui.painter().rect_filled(handle_rect, 2.0, handle_color); 
                                       if is_focused {
                                           ui.painter().rect_stroke(handle_rect, 2.0, egui::Stroke::new(1.0, color_main), egui::StrokeKind::Middle);
                                       }
                                       
                                       // Tooltip Zero-Latency
                                       if response.hovered() || response.dragged() || is_focused {
                                           let val_display = Self::format_value(gain);
                                           let tooltip_pos = egui::pos2(handle_rect.right() + 8.0, handle_rect.center().y);
                                           let painter = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Tooltip, response.id));
                                           let galley = ui.painter().layout_no_wrap(val_display, egui::FontId::proportional(11.0), color_main);
                                           let text_rect = egui::Rect::from_min_size(tooltip_pos - egui::vec2(0.0, galley.size().y / 2.0), galley.size());
                                           let bg_rect = text_rect.expand(4.0);
                                           painter.rect_filled(bg_rect, 2.0, color_contrast);
                                           painter.rect_stroke(bg_rect, 2.0, egui::Stroke::new(1.0, color_accent), egui::StrokeKind::Middle);
                                           painter.galley(text_rect.min, galley, color_main);
                                       }
                                       
                                       ui.add_space(5.0);
                                       
                                       if !show_top {
                                            // Bottom Label (Fixed Height Space? No, default align)
                                            // Just label
                                            ui.label(egui::RichText::new(freq_label).size(10.0).color(color_main));
                                       } else {
                                            ui.allocate_space(egui::vec2(1.0, 13.0)); // Placeholder height
                                       }
                                       // dB Label Removed
                                   });
                                   
                                   ui.add_space(18.0); 
                               }
                           });
                       } else {
                           ui.horizontal(|ui| {
                               ui.add_space(-12.0); // Padding left
                               
                               let band_count = audio_manager.get_eq_bands_count();
                               if band_count != 31 {
                                   audio_manager.set_eq_mode(31);
                               }

                               for i in 0..31 {
                                   let info = audio_manager.get_eq_band_info(i).unwrap_or((0.0, 0.0));
                                   let freq = info.0;
                                   let mut gain = info.1;
                                   
                                   ui.vertical(|ui| {
                                       // Fixed Header
                                       let (rect_header, _) = ui.allocate_exact_size(egui::vec2(14.0, header_height), egui::Sense::hover());

                                       // Alternating Labels
                                       let show_top = i % 2 == 0;
                                       let freq_label = if freq >= 1000.0 { format!("{:.1}", freq/1000.0).replace(".0", "") } else { format!("{:.0}", freq) };
                                       
                                       if show_top {
                                            ui.painter().text(rect_header.center(), egui::Align2::CENTER_CENTER, freq_label.clone(), egui::FontId::proportional(10.0), color_main);
                                       }
                                       ui.add_space(13.0);

                                       // Custom Vertical Slider
                                       let slider_size = egui::vec2(14.0, slider_height);
                                       let (rect, _) = ui.allocate_exact_size(slider_size, egui::Sense::hover());
                                       let mut response = ui.interact(rect, ui.id().with("eq_band_31_slider_id").with(i), egui::Sense::click_and_drag());
                                       
                                       // Interaction
                                       let slider_id = format!("eq_31_{}", i);
                                       let is_focused = ui_state.focused_slider.as_deref() == Some(&slider_id);
                                       if response.clicked() || response.dragged() {
                                           ui_state.focused_slider = Some(slider_id);
                                       }
                                       // Reset on right click
                                       if response.clicked_by(egui::PointerButton::Secondary) {
                                           gain = 0.0;
                                           audio_manager.set_eq_band_gain(i, gain);
                                           response.mark_changed();
                                       } else if response.dragged() || response.clicked() {
                                           if let Some(pos) = response.interact_pointer_pos() {
                                               let t = 1.0 - (pos.y - rect.min.y) / rect.height();
                                               // Range +/- 9dB
                                               let val = -9.0 + t.clamp(0.0, 1.0) * 18.0; 
                                               // Snap to 0.1
                                               let val_snapped = (val * 10.0).round() / 10.0;
                                               
                                               if (gain - val_snapped).abs() > 0.001 {
                                                    gain = val_snapped;
                                                    audio_manager.set_eq_band_gain(i, gain);
                                                    response.mark_changed();
                                               }
                                           }
                                       }
                                       
                                       // Keyboard
                                       if is_focused {
                                           let mut step = 0.0;
                                           ui.input_mut(|inp| {
                                               if inp.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) { step = 0.1; }
                                               else if inp.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) { step = -0.1; }
                                           });
                                           if step != 0.0 {
                                               let val_snapped = (gain + step).clamp(-9.0, 9.0);
                                               let val_snapped = (val_snapped * 10.0).round() / 10.0;
                                               if (gain - val_snapped).abs() > 0.001 {
                                                    gain = val_snapped;
                                                    audio_manager.set_eq_band_gain(i, gain);
                                                    response.mark_changed();
                                               }
                                           }
                                       }
                                       
                                       // Draw Track
                                       let track_width = 8.0;
                                       let track_rect = egui::Rect::from_center_size(rect.center(), egui::vec2(track_width, rect.height()));
                                       ui.painter().rect_filled(track_rect, 2.0, color_contrast);
                                       
                                       // Draw Handle
                                       // Range -9 to +9
                                       let curr_t = (gain - (-9.0)) / 18.0;
                                       let handle_y = rect.max.y - curr_t * rect.height();
                                       let handle_size = egui::vec2(14.0, 16.0); // 14x16 keeping consistent
                                       let handle_rect = egui::Rect::from_center_size(egui::pos2(rect.center().x, handle_y), handle_size);
                                       
                                       let handle_color = if ui_state.equalizer_enabled { color_accent } else { color_contrast };
                                       ui.painter().rect_filled(handle_rect, 2.0, handle_color); 
                                       if is_focused {
                                           ui.painter().rect_stroke(handle_rect, 2.0, egui::Stroke::new(1.0, color_main), egui::StrokeKind::Middle);
                                       }
                                       
                                       // Tooltip Zero-Latency
                                       if response.hovered() || response.dragged() || is_focused {
                                           let val_display = Self::format_value(gain);
                                           let tooltip_pos = egui::pos2(handle_rect.right() + 8.0, handle_rect.center().y);
                                           let painter = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Tooltip, response.id));
                                           let galley = ui.painter().layout_no_wrap(val_display, egui::FontId::proportional(11.0), color_main);
                                           let text_rect = egui::Rect::from_min_size(tooltip_pos - egui::vec2(0.0, galley.size().y / 2.0), galley.size());
                                           let bg_rect = text_rect.expand(4.0);
                                           painter.rect_filled(bg_rect, 2.0, color_contrast);
                                           painter.rect_stroke(bg_rect, 2.0, egui::Stroke::new(1.0, color_accent), egui::StrokeKind::Middle);
                                           painter.galley(text_rect.min, galley, color_main);
                                       }
                                       
                                       ui.add_space(5.0);

                                       if !show_top {
                                            ui.label(egui::RichText::new(freq_label).size(10.0).color(color_main));
                                       } else {
                                            ui.allocate_space(egui::vec2(1.0, 13.0));
                                       }
                                       // dB Label Removed
                                   });
                                   
                                   ui.add_space(6.1); // 5px spacing as requested
                               }
                           });
                       }
                  });
             });
        });
    }

// Helper outside impl
fn format_value(v: f32) -> String {
    if v.abs() < 0.05 { "0".to_string() }
    else { format!("{:.1}", v).trim_end_matches(".0").to_string() }
}




    fn tab_audio_effects(
        ui: &mut egui::Ui,
        _ui_state: &mut AudioCenterState,
        color_accent: egui::Color32,
        color_main: egui::Color32,
        color_contrast: egui::Color32,
        color_sec: egui::Color32,
        audio_manager: &AudioManager,
    ) {
        egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
            ui.horizontal_top(|ui| {
                let col_width = (ui.available_width() - 40.0) / 3.0; // 3 columns
                
                // Column 1: Base & Frequencies
                ui.allocate_ui_with_layout(egui::vec2(col_width, ui.available_height()), egui::Layout::top_down(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new("Base y Frecuencias").color(color_accent).strong());
                    ui.add_space(10.0);
                    
                    Self::draw_effect_control(ui, "Aumento de Graves", color_main, color_contrast, color_accent, color_sec, audio_manager,
                        |dsp| dsp.bass_boost.enabled,
                        |dsp, v| dsp.bass_boost.enabled = v,
                        "Nivel", |dsp| dsp.bass_boost.gain, |dsp, v| dsp.bass_boost.gain = v, 0.0..=24.0, 0.0);
                    
                    Self::draw_effect_control(ui, "ReplayGain", color_main, color_contrast, color_accent, color_sec, audio_manager,
                        |dsp| dsp.replay_gain.enabled,
                        |dsp, v| dsp.replay_gain.enabled = v,
                        "Pre-amp (dB)", |dsp| dsp.replay_gain.preamp, |dsp, v| dsp.replay_gain.preamp = v, -12.0..=12.0, 0.0);
                        
                    Self::draw_effect_control(ui, "Compresor", color_main, color_contrast, color_accent, color_sec, audio_manager,
                        |dsp| dsp.compressor.enabled,
                        |dsp, v| dsp.compressor.enabled = v,
                        "Umbral (dB)", |dsp| dsp.compressor.threshold, |dsp, v| dsp.compressor.threshold = v, -60.0..=0.0, -10.0);
                        
                    Self::draw_effect_control(ui, "Túnel 3D (HRTF)", color_main, color_contrast, color_accent, color_sec, audio_manager,
                        |dsp| dsp.hrtf.enabled,
                        |dsp, v| dsp.hrtf.enabled = v,
                        "Mix", |_| 100.0, |_, _| {}, 0.0..=100.0, 100.0); // Placeholder mix
                        
                    Self::draw_effect_control(ui, "Flanger", color_main, color_contrast, color_accent, color_sec, audio_manager,
                        |dsp| dsp.flanger.enabled,
                        |dsp, v| dsp.flanger.enabled = v,
                        "Mix", |dsp| dsp.flanger.mix, |dsp, v| dsp.flanger.mix = v, 0.0..=1.0, 0.5);
                });
                
                ui.add_space(20.0);
                
                // Column 2: Voices, Noise & Modulation
                ui.allocate_ui_with_layout(egui::vec2(col_width, ui.available_height()), egui::Layout::top_down(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new("Voces y Modulación").color(color_accent).strong());
                    ui.add_space(10.0);
                    
                    Self::draw_effect_control(ui, "Aumento de Voces", color_main, color_contrast, color_accent, color_sec, audio_manager,
                        |dsp| dsp.voice_boost.enabled,
                        |dsp, v| dsp.voice_boost.enabled = v,
                        "Nivel", |dsp| dsp.voice_boost.gain, |dsp, v| dsp.voice_boost.gain = v, -3.0..=24.0, 0.0);
                        
                    Self::draw_effect_control(ui, "Reducción de Ruido", color_main, color_contrast, color_accent, color_sec, audio_manager,
                        |dsp| dsp.noise_gate.enabled,
                        |dsp, v| dsp.noise_gate.enabled = v,
                        "Umbral (dB)", |dsp| dsp.noise_gate.threshold, |dsp, v| dsp.noise_gate.threshold = v, -90.0..=0.0, -60.0);
                        
                    Self::draw_effect_control(ui, "Limitador", color_main, color_contrast, color_accent, color_sec, audio_manager,
                        |dsp| dsp.limiter.enabled,
                        |dsp, v| dsp.limiter.enabled = v,
                        "Techo (dB)", |dsp| dsp.limiter.ceiling, |dsp, v| dsp.limiter.ceiling = v, -6.0..=0.0, -0.1);
                        
                    Self::draw_effect_control(ui, "Delay", color_main, color_contrast, color_accent, color_sec, audio_manager,
                        |dsp| dsp.delay.enabled,
                        |dsp, v| dsp.delay.enabled = v,
                        "Tiempo (s)", |dsp| dsp.delay.time, |dsp, v| dsp.delay.time = v, 0.01..=2.0, 0.3);
                        
                    Self::draw_effect_control(ui, "Phaser", color_main, color_contrast, color_accent, color_sec, audio_manager,
                        |dsp| dsp.phaser.enabled,
                        |dsp, v| dsp.phaser.enabled = v,
                        "Mix", |dsp| dsp.phaser.mix, |dsp, v| dsp.phaser.mix = v, 0.0..=1.0, 0.5);
                });
                
                ui.add_space(20.0);
                
                // Column 3: Space & Stereo Mix
                ui.allocate_ui_with_layout(egui::vec2(col_width, ui.available_height()), egui::Layout::top_down(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new("Espacio y Estéreo").color(color_accent).strong());
                    ui.add_space(10.0);
                    
                    Self::draw_effect_control(ui, "Expansor Estéreo", color_main, color_contrast, color_accent, color_sec, audio_manager,
                        |dsp| dsp.stereo_expander.enabled,
                        |dsp, v| dsp.stereo_expander.enabled = v,
                        "Ancho (%)", |dsp| dsp.stereo_expander.width * 100.0, |dsp, v| dsp.stereo_expander.width = v / 100.0, 0.0..=200.0, 100.0);
                        
                    Self::draw_effect_control(ui, "Crossfeed", color_main, color_contrast, color_accent, color_sec, audio_manager,
                        |dsp| dsp.crossfeed.enabled,
                        |dsp, v| dsp.crossfeed.enabled = v,
                        "Nivel", |dsp| dsp.crossfeed.amount, |dsp, v| dsp.crossfeed.amount = v, 0.0..=1.0, 0.0);
                        
                    Self::draw_effect_control(ui, "Reverberación", color_main, color_contrast, color_accent, color_sec, audio_manager,
                        |dsp| dsp.reverb.enabled,
                        |dsp, v| dsp.reverb.enabled = v,
                        "Wet", |dsp| dsp.reverb.wet, |dsp, v| dsp.reverb.set_wet(v), 0.0..=1.0, 0.3);
                        
                    Self::draw_effect_control(ui, "Chorus", color_main, color_contrast, color_accent, color_sec, audio_manager,
                        |dsp| dsp.chorus.enabled,
                        |dsp, v| dsp.chorus.enabled = v,
                        "Prof.", |dsp| dsp.chorus.depth, |dsp, v| dsp.chorus.depth = v, 0.0..=1.0, 0.5);
                        
                    Self::draw_effect_control(ui, "Balance Estéreo", color_main, color_contrast, color_accent, color_sec, audio_manager,
                        |dsp| dsp.stereo_balance.enabled,
                        |dsp, v| dsp.stereo_balance.enabled = v,
                        "L/R", |dsp| dsp.stereo_balance.balance, |dsp, v| dsp.stereo_balance.balance = v, -1.0..=1.0, 0.0);
                });
            });
        });
    }

    fn draw_effect_control<FGetEn, FSetEn, FGetVal, FSetVal>(
        ui: &mut egui::Ui,
        title: &str,
        color_main: egui::Color32,
        color_contrast: egui::Color32,
        color_accent: egui::Color32,
        color_sec: egui::Color32,
        audio_manager: &AudioManager,
        get_enabled: FGetEn,
        set_enabled: FSetEn,
        primary_param_name: &str,
        get_val: FGetVal,
        set_val: FSetVal,
        val_range: std::ops::RangeInclusive<f32>,
        default_val: f32,
    ) where 
        FGetEn: Fn(&crate::audio::dsp::DspChain) -> bool,
        FSetEn: Fn(&mut crate::audio::dsp::DspChain, bool),
        FGetVal: Fn(&crate::audio::dsp::DspChain) -> f32,
        FSetVal: Fn(&mut crate::audio::dsp::DspChain, f32),
    {
        ui.vertical(|ui| {
            let stroke_color = if audio_manager.with_dsp(&get_enabled) { color_accent } else { color_contrast };
            
            let frame = egui::Frame::NONE
                .fill(color_contrast)
                .corner_radius(8.0)
                .inner_margin(egui::Margin::symmetric(10, 8))
                .stroke(egui::Stroke::new(1.0, stroke_color));
                
            let is_enabled = audio_manager.with_dsp(&get_enabled);
            
            frame.show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(title).size(13.0).color(color_main));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let mut enabled_mut = is_enabled;
                        if Self::custom_toggle(ui, &mut enabled_mut, color_accent, color_contrast, color_sec).changed() {
                            audio_manager.with_dsp_mut(|dsp| set_enabled(dsp, enabled_mut));
                        }
                    });
                });
                
                ui.add_space(5.0);
                
                // Primary Slider
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(primary_param_name).size(11.0).color(color_sec));
                    
                    // Num Edit will be on the right, Slider will take remaining space
                    let edit_width = 40.0;
                    let slider_width = ui.available_width() - edit_width - 10.0; 
                    let slider_size = egui::vec2(slider_width, 20.0);
                    
                    let (rect, mut response) = ui.allocate_exact_size(slider_size, egui::Sense::click_and_drag());
                    let id = response.id;
                    
                    if response.clicked() || response.dragged() {
                        ui.memory_mut(|m| m.request_focus(id));
                    }
                    let is_focused = ui.memory(|m| m.has_focus(id));
                    
                    // Right click reset
                    if response.clicked_by(egui::PointerButton::Secondary) {
                        audio_manager.with_dsp_mut(|dsp| set_val(dsp, default_val));
                        response.mark_changed();
                    } else if response.dragged() || response.clicked() {
                        if let Some(pos) = response.interact_pointer_pos() {
                            let t = (pos.x - rect.min.x) / rect.width();
                            let t_clamped = t.clamp(0.0, 1.0);
                            let min_v = *val_range.start();
                            let max_v = *val_range.end();
                            let mut val = min_v + t_clamped * (max_v - min_v);
                            val = (val * 100.0).round() / 100.0;
                            
                            audio_manager.with_dsp_mut(|dsp| set_val(dsp, val));
                            response.mark_changed();
                        }
                    }
                    
                    // Keyboard arrows handling
                    if is_focused {
                        let mut step = 0.0;
                        ui.input_mut(|i| {
                            if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight) {
                                step = (*val_range.end() - *val_range.start()) * 0.01;
                            } else if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft) {
                                step = -((*val_range.end() - *val_range.start()) * 0.01);
                            }
                        });
                        
                        if step != 0.0 {
                            let curr = audio_manager.with_dsp(&get_val);
                            let mut new_val = (curr + step).clamp(*val_range.start(), *val_range.end());
                            new_val = (new_val * 100.0).round() / 100.0;
                            audio_manager.with_dsp_mut(|dsp| set_val(dsp, new_val));
                            response.mark_changed();
                        }
                    }
                    
                    // Drawing Slider
                    let val = audio_manager.with_dsp(&get_val);
                    let min_v = *val_range.start();
                    let max_v = *val_range.end();
                    let t = if max_v > min_v { ((val - min_v) / (max_v - min_v)).clamp(0.0, 1.0) } else { 0.0 };
                    
                    // Track Black #000000
                    let track_height = 8.0;
                    let track_rect = egui::Rect::from_center_size(rect.center(), egui::vec2(rect.width(), track_height));
                    ui.painter().rect_filled(track_rect, 2.0, egui::Color32::from_rgb(0, 0, 0));
                    
                    // Colored trailing fill
                    if is_enabled && t > 0.0 {
                        let mut filled_rect = track_rect;
                        filled_rect.max.x = rect.min.x + t * rect.width();
                        ui.painter().rect_filled(filled_rect, 2.0, color_accent);
                    }
                    
                    // Handle Match PreAmp (14x16)
                    let handle_x = rect.min.x + t * rect.width();
                    let handle_size = egui::vec2(14.0, 16.0);
                    let handle_rect = egui::Rect::from_center_size(egui::pos2(handle_x, rect.center().y), handle_size);
                    
                    let handle_color = if is_enabled { color_accent } else { color_contrast };
                    ui.painter().rect_filled(handle_rect, 2.0, handle_color);
                    
                    if is_focused {
                        ui.painter().rect_stroke(handle_rect, 2.0, egui::Stroke::new(1.0, color_main), egui::StrokeKind::Middle);
                    }

                    // Input Text Box
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let mut val_str = format!("{:.2}", val);
                        let response = ui.add(egui::TextEdit::singleline(&mut val_str).desired_width(edit_width).text_color(color_main));
                        if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            if let Ok(parsed) = val_str.parse::<f32>() {
                                let clamped = parsed.clamp(*val_range.start(), *val_range.end());
                                audio_manager.with_dsp_mut(|dsp| set_val(dsp, clamped));
                            }
                        }
                    });
                });
            });
            ui.add_space(10.0);
        });
    }

    fn custom_toggle(ui: &mut egui::Ui, on: &mut bool, color_accent: egui::Color32, color_contrast: egui::Color32, color_sec: egui::Color32) -> egui::Response {
        let desired_size = egui::vec2(34.0, 17.0);
        let (rect, mut response) = ui.allocate_exact_size(desired_size, egui::Sense::click());
        if response.clicked() {
            *on = !*on;
            response.mark_changed();
        }
        response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, true, *on, ""));

        if ui.is_rect_visible(rect) {
            let how_on = ui.ctx().animate_bool_responsive(response.id, *on);
            
            let fill = if *on { color_accent } else { color_contrast };
            
            ui.painter().rect(
                rect,
                rect.height() / 2.0,
                fill,
                egui::Stroke::NONE,
                egui::StrokeKind::Middle
            );
            
            let circle_x = egui::lerp((rect.left() + rect.height() / 2.0)..=(rect.right() - rect.height() / 2.0), how_on);
            let center = egui::pos2(circle_x, rect.center().y);
            let radius = rect.height() / 2.0 - 2.0;
            // White knob or dark depending on background
            let knob_color = if *on { color_contrast } else { color_sec };
            ui.painter().circle_filled(center, radius, knob_color);
        }

        response
    }
}

fn restart_audio_service() {
    std::thread::spawn(|| {
         // Detectar PipeWire
         let check_pw = Command::new("systemctl")
             .args(&["--user", "is-active", "pipewire"])
             .output();
         
         let has_pw = match check_pw {
             Ok(out) => String::from_utf8_lossy(&out.stdout).trim() == "active",
             Err(_) => false,
         };

         let cmd = if has_pw {
             "systemctl --user restart pipewire wireplumber"
         } else {
             "systemctl --user restart pulseaudio"
         };

         tracing::info!("Reiniciando servicio de audio: {}", cmd);
         let _ = Command::new("sh").arg("-c").arg(cmd).spawn();
    });
}


