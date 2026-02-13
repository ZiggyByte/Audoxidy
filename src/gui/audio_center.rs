use eframe::egui;
use crate::audio::engine::{AudioSettings, BitDepth, ChannelConfig, AudioDeviceInfo};
use crate::audio::AudioManager;
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
    show_hifi_warning: bool,
    
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
    
    // UI Helpers
    hifi_pending_activation: bool,
    apply_enabled: bool, // Para activar botón "Aplicar" solo si hay cambios
}

impl Default for AudioCenterState {
    fn default() -> Self {
        Self {
            selected_tab: 0,
            show_hifi_warning: false,
            cached_hosts: Vec::new(),
            cached_devices: Vec::new(),
            selected_host: None,
            selected_device: None,
            selected_sample_rate: None,
            selected_bit_depth: None,
            selected_buffer_size: None,
            channels_auto: true,
            selected_channels_manual: 2,
            hifi_pending_activation: false,
            apply_enabled: false,
        }
    }
}

const TAB_NAMES: [&str; 3] = ["Configuración de Audio", "Ecualizador", "Efectos de Audio"];

impl AudioCenter {
    pub fn show(&mut self, ctx: &egui::Context, audio_manager: &AudioManager) {
        if !self.open { 
            self.first_open = true;
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
        let color_bg = crate::gui::theme::COLOR_BG;
        let color_accent = crate::gui::theme::COLOR_ACCENT; // #FF003D
        let color_contrast = crate::gui::theme::COLOR_CONTRAST; // #111111
        let color_text_main = crate::gui::theme::COLOR_TEXT_PRIMARY; // #c6c6c6
        let color_text_sec = crate::gui::theme::COLOR_TEXT_SECONDARY; // #717171

        let window_size = egui::vec2(864.0, 474.0);
        let screen_rect = ctx.input(|i| i.content_rect());
        let pos = screen_rect.center() - window_size / 2.0;

        // Area de bloqueo para modal (si popup activo)
        let interact_enabled = !ui_state.show_hifi_warning;

        egui::Window::new("AudioCenter")
            .fixed_size(window_size)
            .fixed_pos(pos)
            .collapsible(false)
            .resizable(false)
            .title_bar(false)
            .frame(egui::Frame::NONE.fill(color_bg))
            .show(ctx, |ui| {
                ui.add_enabled_ui(interact_enabled, |ui| {
                    let rect = ui.max_rect();
                    ui.painter().rect_filled(rect, 0.0, color_bg);

                    // --- Barra de Título (34px) ---
                    let header_rect = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), 34.0));
                    
                    // Título Centrado
                    ui.painter().text(
                        header_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "Centro de Audio Avanzado",
                        egui::FontId::proportional(16.0),
                        color_text_main
                    );

                    // Botón Cerrar (Alineado a la derecha)
                    let close_size = 20.0;
                    let close_rect = egui::Rect::from_center_size(
                        egui::pos2(header_rect.max.x - 25.0, header_rect.center().y),
                        egui::vec2(close_size, close_size),
                    );
                    
                    if ui.interact(close_rect, ui.id().with("close_btn"), egui::Sense::click()).clicked() {
                        is_open = false;
                    }
                    ui.painter().text(close_rect.center(), egui::Align2::CENTER_CENTER, "X", egui::FontId::proportional(16.0), color_text_main);

                    // --- Contenido (Padding 15px) ---
                    let content_rect = rect.shrink(15.0);
                    let content_rect = egui::Rect::from_min_max(
                        egui::pos2(content_rect.min.x, header_rect.max.y), 
                        content_rect.max
                    );

                    ui.scope_builder(egui::UiBuilder::new().max_rect(content_rect), |ui| {
                        ui.add_space(10.0);

                        // --- Pestañas ---
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(10.0, 0.0); // Espacio entre pestañas 10px
                            for (i, name) in TAB_NAMES.iter().enumerate() {
                                let is_selected = ui_state.selected_tab == i;
                                let text_color = if is_selected { egui::Color32::WHITE } else { color_text_main };
                                let bg_color = if is_selected { color_accent } else { egui::Color32::TRANSPARENT };
                                
                                // Padding interno 10px
                                if ui.add(egui::Button::new(egui::RichText::new(*name).color(text_color).size(14.0))
                                    .fill(bg_color)
                                    .frame(true)
                                    .min_size(egui::vec2(0.0, 30.0))
                                    .corner_radius(0.0)
                                ).clicked() {
                                    ui_state.selected_tab = i;
                                }
                            }
                        });

                        // Borde divisor
                        let divider_rect = egui::Rect::from_min_size(
                            ui.cursor().min + egui::vec2(0.0, 5.0),
                            egui::vec2(ui.available_width(), 1.0)
                        );
                        ui.painter().rect_filled(divider_rect, 0.0, color_contrast);
                        ui.add_space(15.0);

                        // --- Contenido de Pestaña ---
                        match ui_state.selected_tab {
                            0 => Self::tab_audio_config(ui, audio_manager, ui_state, color_accent, color_text_main, color_text_sec, color_contrast),
                            1 => Self::tab_placeholder(ui, "Ecualizador", audio_manager, color_accent, color_text_main),
                            2 => Self::tab_placeholder(ui, "Efectos de Audio", audio_manager, color_accent, color_text_main),
                            _ => {}
                        }
                    });
                });
            });
        
        // --- Popup Modal Hi-Fi ---
        if ui_state.show_hifi_warning {
            // Fondo eliminado por peticion del usuario ("Elimina el fondo semitransparente")
            // Solo mostramos el popup
            Self::show_hifi_warning_popup(ctx, ui_state, audio_manager, color_bg, color_accent, color_text_main);
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
        if self.state.selected_sample_rate.is_none() {
             self.state.selected_sample_rate = Some(44100);
        }
        
        // Channels logic
        if s.channels > 2 {
             self.state.selected_channels_manual = s.channels;
             self.state.channels_auto = false;
        } else {
             self.state.channels_auto = true; 
        }

        self.state.selected_buffer_size = if s.buffer_size > 0 { Some(s.buffer_size) } else { None };
        
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
        let is_hifi_active = audio_manager.state().read().bit_perfect;
        
        let available_width = ui.available_width();
        let left_width = available_width * 0.60;
        let right_width = available_width - left_width - 20.0; // Spacing logic

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
                            let label_width = 160.0;
                            ui.allocate_ui(egui::vec2(label_width, 20.0), |ui| {
                                ui.label(egui::RichText::new(label).color(color).strong());
                            });
                            ui.add_space(10.0);
                            content(ui);
                         });
                         changed
                    }

                    // Servidor de Audio
                    ui.add_enabled_ui(!is_hifi_active, |ui| {
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
                            .width(212.0)
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
                    ui.add_enabled_ui(!is_hifi_active, |ui| {
                        setting_row_aligned(ui, "Frecuencia de Muestreo:", color_main, |ui| {
                            let mut selected = ui_state.selected_sample_rate;
                            let prev = selected;

                            egui::ComboBox::from_id_salt("rate_combo")
                                .width(126.0)
                                .selected_text(selected.map(|r| format!("{} Hz", r)).unwrap_or("Auto".into()))
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut selected, None, "Auto");
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
                                    None => "Auto"
                                })
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut selected, None, "Auto");
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
                            let mut auto = ui_state.channels_auto;
                            let mut manual = ui_state.selected_channels_manual;
                            let prev_auto = auto;
                            let prev_manual = manual;
                            
                             if !auto {
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
                             }
                             
                             ui.add_space(5.0);
                             if ui.add(toggle_ui(&mut auto)).changed() { }
                             ui.add_space(5.0);
                             ui.label("Auto");

                             if auto != prev_auto {
                                 ui_state.channels_auto = auto;
                                 ui_state.apply_enabled = true;
                             }
                             if manual != prev_manual {
                                 ui_state.selected_channels_manual = manual;
                                 ui_state.apply_enabled = true;
                             }
                        });

                        // Quantum / Buffer Size
                        setting_row_aligned(ui, "Quantum (Buffer):", color_main, |ui| {
                            let mut selected = ui_state.selected_buffer_size;
                            let prev = selected;
                            
                            egui::ComboBox::from_id_salt("buffer_combo")
                                .width(126.0)
                                .selected_text(selected.map(|b| format!("{}", b)).unwrap_or("Auto".into()))
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut selected, None, "Auto");
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

                    // Motor HI-FI
                    // Diseño solicitado: En un solo renglón leyenda "Motor Hi-Fi..." y icono grande.
                    // Debajo "Bit-Perfect:" y Switch.
                    // Sin separador.
                    
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Motor Hi-Fi ").color(color_accent).size(18.0).strong()); // Use size or proportional
                        ui.label(egui::RichText::new(" (Motor exclusivo disponible solo en Linux)").color(color_accent).size(14.0));
                        ui.add_space(5.0);
                        let icon = if is_hifi_active {
                            egui::include_image!("../../assets/icons/rocket-launch-fill.svg")
                        } else {
                            egui::include_image!("../../assets/icons/rocket-launch-outlined.svg")
                        };
                        // Icono más grande
                        ui.add(egui::Image::new(icon).max_height(36.0).tint(color_accent));
                    });
                    
                    ui.add_space(-7.0);
                    
                    ui.horizontal(|ui| {
                         ui.label(egui::RichText::new("*Bit-Perfect:").color(color_accent).size(17.0).strong());
                         ui.add_space(10.0);
                         let mut hifi_check = is_hifi_active;
                         if ui.add(toggle_ui(&mut hifi_check)).changed() {
                            if hifi_check {
                                ui_state.hifi_pending_activation = true;
                                ui_state.show_hifi_warning = true;
                            } else {
                                // Deactivate
                                // Se debe aplicar "inmediato"??
                                // En el código anterior era inmediato.
                                // Mantengamos inmediato o via botón aplicar?
                                // "toggle switch de activar y desactivar... en el popup de confirmación".
                                // Si desactivo, no hay popup.
                                // Aplicamos inmediatamente al desactivar? 
                                // O habilitamos el botón Aplicar.
                                // El usuario dijo "botones aplicar... desactivado para que no se pueda dar clic y solo de activarse automaticamente... cuando se realiza un cambio".
                                // Entonces Hi-Fi toggle también debe respetar botón Aplicar?
                                // PERO el popup Hi-Fi tiene botón "Aceptar" que *activa*.
                                // Flujo mixto: Activar -> Popup -> Aceptar -> ACTIVA YA.
                                // Desactivar -> Toggle Off -> Habilitar "Aplicar"?
                                // Vamos a hacer que Desactivar habilite el botón "Aplicar" con bit_perfect = false.
                                // Y Activar -> Popup -> Pone bit_perfect = true en settings pendientes y habilita Aplicar?
                                // O Activa directo?
                                // El popup dice "A punto de activar...". Botón "Aceptar".
                                // Si da aceptar, lo lógico es activar.
                                
                                // Logic:
                                // Activar -> Show Popup. Popup Accept -> Call apply immediately inside Popup logic?
                                // Or Set pending state and Enable Apply button?
                                // "El botón aplicar debe estar como desactivado... activarse... cuando se realiza un cambio".
                                
                                // Si sigo el flujo del botón Aplicar:
                                // Popup Aceptar -> Cierra popup, UiState.hifi_on = true, ApplyButton = enabled.
                                // Apply Click -> Sends settings.
                                
                                // Let's try that for consistency?
                                // But Hi-Fi warning implies big change.
                                // Previous code applied immediately.
                                // Let's keep Immediate for Activation via Popup for safety, but Deactivation via Apply button?
                                // Or uniform: All via Apply.
                                
                                // Current UI renders toggle state based on `is_hifi_active` (Engine state).
                                // If I click toggle, I expect visual change locally.
                                // So I need local hifi state in ui_state?
                                // No, I can rely on `apply_enabled` and local vars if needed.
                                
                                // For simplicity and robustness (as user asked for fixes):
                                // Keep Activation Immediate via Popup (it's a heavy mode).
                                // Deactivation Immediate?
                                // Let's stick to Immediate for Hi-Fi to avoid confusion.
                                
                                let settings = AudioSettings {
                                    host_id: ui_state.selected_host.clone(),
                                    device_name: ui_state.selected_device.clone(),
                                    sample_rate: ui_state.selected_sample_rate,
                                    bit_depth: ui_state.selected_bit_depth,
                                    channels: if ui_state.channels_auto { ChannelConfig::Auto } else { ChannelConfig::Manual(ui_state.selected_channels_manual) },
                                    bit_perfect: false,
                                    buffer_size: ui_state.selected_buffer_size,
                                };
                                let _ = audio_manager.apply_audio_settings(settings);
                                ui_state.apply_enabled = false;
                            }
                        }
                    });
                }
             );

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
                    let backend_str = if is_hifi_active { "ALSA (Exclusivo)" } else { "Compartido" };

                    let info_rows = [
                        ("Backend:", backend_str), 
                        ("Dispositivo:", if is_hifi_active { "Hw Direct" } else { "Sistema" }), 
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

        // --- Footer Buttons ---
        ui.add_space(10.0);
        
        ui.label(egui::RichText::new("*El motor Hi-Fi envia la señal de audio bit a bit directo a su dispositivo para una pureza de audio absoluta.")
            .color(color_sec).size(12.0));
        
        ui.add_space(25.0);

        ui.horizontal(|ui| {
             ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                 // Estilo personalizado para botones
                 let mut btn_style = (**ui.style()).clone();
                 btn_style.spacing.button_padding = egui::vec2(10.0, 5.0);
                 btn_style.visuals.widgets.hovered.weak_bg_fill = color_accent;
                 btn_style.visuals.widgets.hovered.bg_fill = color_accent;
                 btn_style.visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, color_main); // Texto blanco/main al hover
                 
                 ui.scope(|ui| {
                     ui.set_style(btn_style);
                     
                     if ui.add(egui::Button::new("Reiniciar servicio de Audio").min_size(egui::vec2(0.0, 30.0))).clicked() {
                          restart_audio_service();
                     }
                     
                     ui.add_space(10.0);
                     
                     if ui.add(egui::Button::new("Predeterminado").min_size(egui::vec2(0.0, 30.0))).clicked() {
                         // Reset UI state
                         ui_state.selected_sample_rate = Some(44100);
                         ui_state.selected_bit_depth = Some(BitDepth::Bits32Float); 
                         ui_state.selected_buffer_size = None;
                         ui_state.channels_auto = true;
                         // Server/Device reset? 
                         if ui_state.cached_hosts.contains(&"ALSA".to_string()) {
                            ui_state.selected_host = Some("ALSA".to_string());
                         }
                         let device_list = &ui_state.cached_devices;
                         let target = device_list.iter().find(|d| d.name.to_lowercase().contains("pipewire"))
                             .or_else(|| device_list.iter().find(|d| d.name.to_lowercase().contains("pulse")))
                             .or_else(|| device_list.iter().find(|d| d.name.to_lowercase().contains("default")));
                         if let Some(d) = target { ui_state.selected_device = Some(d.name.clone()); }

                         ui_state.apply_enabled = true;
                     }

                     ui.add_space(10.0);

                     // Boton Aplicar (Desactivado si no cambios)
                     ui.add_enabled_ui(ui_state.apply_enabled, |ui| {
                         if ui.add(egui::Button::new("Aplicar").min_size(egui::vec2(0.0, 30.0))).clicked() {
                              let settings = AudioSettings {
                                  host_id: ui_state.selected_host.clone(),
                                  device_name: ui_state.selected_device.clone(),
                                  sample_rate: ui_state.selected_sample_rate,
                                  bit_depth: ui_state.selected_bit_depth,
                                  channels: if ui_state.channels_auto { ChannelConfig::Auto } else { ChannelConfig::Manual(ui_state.selected_channels_manual) },
                                  bit_perfect: is_hifi_active, 
                                  buffer_size: ui_state.selected_buffer_size, 
                              };
                              let _ = audio_manager.apply_audio_settings(settings);
                              ui_state.apply_enabled = false;
                         }
                     });
                 });
             });
        });
    }

    fn tab_placeholder(ui: &mut egui::Ui, title: &str, _manager: &AudioManager, color_accent: egui::Color32, _color_main: egui::Color32) {
        let is_hifi = _manager.state().read().bit_perfect;
        if is_hifi {
            ui.vertical_centered(|ui| {
                ui.add_space(50.0);
                ui.label(egui::RichText::new("Motor Hi-Fi Activado").heading().color(color_accent));
                ui.label(egui::RichText::new("No se pueden hacer ajustes").color(color_accent));
            });
        } else {
            ui.label(title);
        }
    }

    fn show_hifi_warning_popup(
        ctx: &egui::Context, 
        ui_state: &mut AudioCenterState, 
        manager: &AudioManager,
        bg_color: egui::Color32,
        accent_color: egui::Color32,
        text_color: egui::Color32
    ) {
        egui::Window::new("Advertencia Hi-Fi")
            .collapsible(false)
            .resizable(false)
            .title_bar(false) 
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .fixed_size(egui::vec2(480.0, 300.0))
            .frame(egui::Frame::NONE.fill(bg_color).stroke(egui::Stroke::new(2.0, accent_color)).inner_margin(15.0))
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                ui.painter().rect_filled(rect, 0.0, bg_color);
                
                ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
                    ui.horizontal(|ui| {
                        // Icono warning correcto 48px
                         let icon = egui::include_image!("../../assets/icons/warning-on-outlined.svg");
                         ui.add(egui::Image::new(icon).max_height(48.0).tint(accent_color)); 
                         
                         ui.add_space(10.0);
                         ui.vertical(|ui| {
                             ui.label(egui::RichText::new("Esta a punto de activar el Motor Hi-Fi.").heading().color(accent_color));
                         });
                    });
                    
                    ui.add_space(15.0);
                    ui.add(egui::Label::new(egui::RichText::new(
                        "El reproductor tendrá el control total de su salida de audio, por lo que no escuchara otros audios o videos en su sistema operativo, tampoco no podrá controlar el volumen del audio, aplicar efectos dsp, compresiones, etc. Esto es para garantizar que la señal de audio se envié a la máxima calidad total a su dispositivo de salida.\n\nSe recomienda que antes de activar el Motor Hi-Fi baje el volumen de su dispositivo receptor de audio."
                    ).color(text_color).size(12.0)).wrap());
                    
                    ui.add_space(25.0);
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                             // Aceptar -> Activar Inmediato
                             if ui.button(egui::RichText::new(" Aceptar ").color(egui::Color32::WHITE).background_color(accent_color)).clicked() {
                                 let settings = AudioSettings {
                                     host_id: ui_state.selected_host.clone(),
                                     device_name: ui_state.selected_device.clone(),
                                     sample_rate: ui_state.selected_sample_rate,
                                     bit_depth: ui_state.selected_bit_depth,
                                     channels: if ui_state.channels_auto { ChannelConfig::Auto } else { ChannelConfig::Manual(ui_state.selected_channels_manual) },
                                     bit_perfect: true,
                                     buffer_size: ui_state.selected_buffer_size,
                                 };
                                 
                                 if let Err(e) = manager.apply_audio_settings(settings) {
                                     tracing::error!("Failed to activate Hi-Fi: {}", e);
                                 }
                                 ui_state.show_hifi_warning = false;
                                 ui_state.apply_enabled = false;
                             }
                             
                             ui.add_space(10.0);
                             
                             if ui.button("Cancelar").clicked() {
                                 ui_state.show_hifi_warning = false;
                                 ui_state.hifi_pending_activation = false;
                             }
                        });
                    });
                });
            });
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

// Widget Toggle Switch Personalizado
fn toggle_ui(on: &mut bool) -> impl egui::Widget + '_ {
    move |ui: &mut egui::Ui| {
        let desired_size = ui.spacing().interact_size.y * egui::vec2(2.0, 1.0);
        let (rect, mut response) = ui.allocate_exact_size(desired_size, egui::Sense::click());
        if response.clicked() {
            *on = !*on;
            response.mark_changed();
        }
        response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, true, *on, ""));
        
        if ui.is_rect_visible(rect) {
            let how_on = ui.ctx().animate_bool(response.id, *on);
            let visual = ui.style().interact_selectable(&response, *on); 
            let rect = rect.expand(visual.expansion);
            let radius = 0.5 * rect.height();
            
            ui.painter().rect(
                rect, 
                radius, 
                if *on { crate::gui::theme::COLOR_ACCENT } else { egui::Color32::from_gray(60) }, 
                egui::Stroke::NONE,
                egui::StrokeKind::Middle
            );
            
            let circle_x = egui::lerp((rect.left() + radius)..=(rect.right() - radius), how_on);
            let center = egui::pos2(circle_x, rect.center().y);
            ui.painter().circle(center, 0.75 * radius, egui::Color32::WHITE, egui::Stroke::NONE);
        }
        
        response
    }
}
