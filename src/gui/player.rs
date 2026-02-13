use eframe::egui;
use crate::audio::AudioManager;
use crate::gui::theme::{COLOR_TEXT_PRIMARY, COLOR_CONTRAST};

pub fn show_player(ui: &mut egui::Ui, audio_manager: &AudioManager, audio_center_open: &mut bool) {
    let available_size = ui.available_size();
    let side = available_size.x.min(available_size.y);
    
    // --- CAPA 1: Capa Madre (Ratio 1:1 forzado) ---
    let rect = ui.allocate_exact_size(egui::vec2(side, side), egui::Sense::hover()).0;
    let painter = ui.painter_at(rect);

    // Capturamos los datos necesarios y liberamos el bloqueo de lectura inmediatamente
    // para evitar deadlocks cuando hagamos escrituras (volumen, seek, etc.)
    let (state_snap, album_art) = {
        let state_lock = audio_manager.state();
        let s = state_lock.read();
        (s.clone(), s.album_art.clone())
    };

    // --- CAPA 0: Gestión de Volumen (Rueda del ratón) y Arrastre de Ventana ---
    // Colocamos esto PRIMERO para que los botones (que se añaden después) queden "encima"
    // en el orden de widgets de egui y capturen los clics antes que esta capa de fondo.
    let response = ui.interact(rect, ui.id().with("bg_interact"), egui::Sense::click_and_drag());
    
    if response.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            audio_manager.set_volume(state_snap.volume + scroll * 0.001);
            *VOLUME_FEEDBACK_TIME.lock() = ui.input(|i| i.time);
        }
    }
    
    // Arrastre de ventana (Drag) - Solo si no se está interactuando con otra cosa
    if response.dragged_by(egui::PointerButton::Primary) {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }

    // --- CAPA 2: Carátula del Álbum ---
    if let Some(art_bytes) = album_art {
        let image_source = egui::ImageSource::Bytes {
            uri: "album_art_main".into(), // egui cacheará esto por la URI
            bytes: egui::load::Bytes::from(art_bytes),
        };
        ui.put(rect, egui::Image::new(image_source)
            .fit_to_exact_size(egui::vec2(side, side)));
    } else {
        painter.rect_filled(rect, 0.0, COLOR_CONTRAST);
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "AuDoxiDY",
             egui::FontId::new(side * 0.15, egui::FontFamily::Name("logo".into())),
            COLOR_TEXT_PRIMARY
        );
    }

    // --- CAPA 3: Opacidad del 30% ---
    painter.rect_filled(rect, 0.0, egui::Color32::from_black_alpha(76));

    // --- CAPA 4: Información (10 Divisiones) ---
    let h_unit = side / 10.0;

    // 1/10: Cabecera
    let r1 = egui::Rect::from_min_size(rect.min, egui::vec2(side, h_unit));
    draw_layer4_header(ui, &painter, r1, &state_snap, audio_center_open);

    // 8/10: Título (Marquesina)
    let r8 = egui::Rect::from_min_size(rect.min + egui::vec2(0.0, h_unit * 7.0), egui::vec2(side, h_unit));
    draw_layer4_marquee_text(ui, &painter, r8, &state_snap.title, true);

    // 9/10: Artista y Tiempo
    let r9 = egui::Rect::from_min_size(rect.min + egui::vec2(0.0, h_unit * 8.0), egui::vec2(side, h_unit));
    let time_str = if state_snap.is_playing || state_snap.current_pos_sec > 0.0 {
        format!("{}:{:02}", (state_snap.current_pos_sec / 60.0) as u32, (state_snap.current_pos_sec % 60.0) as u32)
    } else {
        String::new()
    };
    draw_layer4_artist_time(ui, &painter, r9, &state_snap.artist, &time_str);

    // 10/10: Barra de Progreso y Seek
    let r10 = egui::Rect::from_min_size(rect.min + egui::vec2(0.0, h_unit * 9.0), egui::vec2(side, h_unit));
    let progress = if state_snap.total_duration_sec > 0.0 {
        (state_snap.current_pos_sec / state_snap.total_duration_sec) as f32
    } else {
        0.0
    };
    draw_layer4_progress_seek(ui, &painter, r10, progress, audio_manager, state_snap.total_duration_sec as f32);

    // --- CAPA 5: Controles de Audio ---
    let controls_rect = egui::Rect::from_min_max(
        rect.min + egui::vec2(0.0, h_unit),
        rect.min + egui::vec2(side, h_unit * 9.0)
    );
    draw_layer5_transport_zones(ui, controls_rect, audio_manager);

    // (La interacción de fondo se movió al principio)
    
    draw_volume_feedback(ui, &painter, rect, state_snap.volume);
}

fn draw_layer4_header(ui: &mut egui::Ui, _painter: &egui::Painter, rect: egui::Rect, state: &crate::audio::engine::AudioState, audio_center_open: &mut bool) {
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            // Menú Hamburguesa (Abre Audio Center)
            let menu_icon = egui::Image::new(egui::include_image!("../../assets/icons/menu.svg"))
                .tint(COLOR_TEXT_PRIMARY).max_width(24.0).sense(egui::Sense::click());
            if ui.add(menu_icon).clicked() {
                *audio_center_open = !*audio_center_open;
            }

            ui.add_space(20.0);
            
            // Info Canales (Centro)
            // Solo se muestra si no es Estéreo (2.0)
            if state.channels != 2 && state.channels > 0 {
                let chan_text = match state.channels {
                    1 => "Mono",
                    3 => "2.1",
                    4 => "4.0",
                    6 => "5.1",
                    8 => "7.1",
                    _ => "Multi",
                };
                ui.add(egui::Label::new(egui::RichText::new(chan_text)
                    .background_color(egui::Color32::from_black_alpha(100))
                    .color(COLOR_TEXT_PRIMARY)).truncate());
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(10.0);
                // Cerrar
                if ui.add(egui::Image::new(egui::include_image!("../../assets/icons/close-small.svg"))
                    .tint(COLOR_TEXT_PRIMARY).max_width(24.0).sense(egui::Sense::click())).clicked() 
                {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
                // Maximizar
                if ui.add(egui::Image::new(egui::include_image!("../../assets/icons/maximize1.svg"))
                    .tint(COLOR_TEXT_PRIMARY).max_width(24.0).sense(egui::Sense::click())).clicked() 
                {
                    let is_max = ui.input(|i| i.viewport().maximized.unwrap_or(false));
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(!is_max));
                }
                // Minimizar
                if ui.add(egui::Image::new(egui::include_image!("../../assets/icons/minimize.svg"))
                    .tint(COLOR_TEXT_PRIMARY).max_width(24.0).sense(egui::Sense::click())).clicked() 
                {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                }
            });
        });
    });
}

fn draw_layer4_marquee_text(ui: &mut egui::Ui, painter: &egui::Painter, rect: egui::Rect, text: &str, is_title: bool) {
    let font_id = if is_title { egui::FontId::proportional(22.0) } else { egui::FontId::proportional(14.0) };
    let galley = ui.painter().layout(text.to_string(), font_id.clone(), COLOR_TEXT_PRIMARY, f32::INFINITY);
    
    if galley.rect.width() > rect.width() - 20.0 {
        let time = ui.input(|i| i.time);
        let speed = 40.0;
        let total_width = galley.rect.width() + 50.0;
        let offset = (time * speed) % total_width as f64;
        
        painter.with_clip_rect(rect).text(
            rect.left_center() + egui::vec2(10.0 - offset as f32, 0.0),
            egui::Align2::LEFT_CENTER, text, font_id.clone(), COLOR_TEXT_PRIMARY
        );
        painter.with_clip_rect(rect).text(
            rect.left_center() + egui::vec2(10.0 - offset as f32 + total_width, 0.0),
            egui::Align2::LEFT_CENTER, text, font_id, COLOR_TEXT_PRIMARY
        );
        ui.ctx().request_repaint();
    } else {
        painter.text(
            rect.left_center() + egui::vec2(10.0, 0.0),
            egui::Align2::LEFT_CENTER, text, font_id, COLOR_TEXT_PRIMARY
        );
    }
}

fn draw_layer4_artist_time(ui: &mut egui::Ui, painter: &egui::Painter, rect: egui::Rect, artist: &str, time_str: &str) {
    let font_id = egui::FontId::proportional(14.0);
    // El artista también puede tener marquesina si es largo
    let galley = ui.painter().layout(artist.to_string(), font_id.clone(), COLOR_TEXT_PRIMARY, f32::INFINITY);
    
    if galley.rect.width() > rect.width() * 0.7 {
        draw_layer4_marquee_text(ui, painter, rect, artist, false);
    } else {
        painter.text(rect.left_center() + egui::vec2(10.0, 0.0), egui::Align2::LEFT_CENTER, artist, font_id, COLOR_TEXT_PRIMARY);
    }
    
    // Tiempo alineado a la derecha
    painter.text(
        rect.right_center() - egui::vec2(10.0, 0.0),
        egui::Align2::RIGHT_CENTER,
        time_str,
        egui::FontId::proportional(14.0),
        COLOR_TEXT_PRIMARY
    );
}

fn draw_layer4_progress_seek(ui: &mut egui::Ui, painter: &egui::Painter, rect: egui::Rect, progress: f32, audio_manager: &AudioManager, duration: f32) {
    // Fondo desenfoque (Simulado con rectángulo semi-transparente)
    painter.rect_filled(rect, 0.0, egui::Color32::from_black_alpha(150));
    
    let bar_rect = egui::Rect::from_center_size(rect.center(), egui::vec2(rect.width() - 20.0, 2.0));
    painter.rect_filled(bar_rect, 1.0, COLOR_CONTRAST);
    
    let mut prog_paint = bar_rect;
    prog_paint.set_width(bar_rect.width() * progress.clamp(0.0, 1.0));
    painter.rect_filled(prog_paint, 1.0, COLOR_TEXT_PRIMARY); // INTERFACE.md dice COLOR_TEXT_PRIMARY al avanzar

    // Interacción Seek
    let response = ui.interact(rect, ui.id().with("seek"), egui::Sense::click_and_drag());
    if response.clicked() || response.dragged() {
        if let Some(pos) = response.interact_pointer_pos() {
            let relative_x = (pos.x - bar_rect.left()).clamp(0.0, bar_rect.width());
            let new_progress = relative_x / bar_rect.width();
            let new_pos = new_progress * duration;
            audio_manager.seek(new_pos as f64);
        }
    }
}

fn draw_layer5_transport_zones(ui: &mut egui::Ui, rect: egui::Rect, audio_manager: &AudioManager) {
    let zone_w = rect.width() / 3.0;
    
    // Zona Anterior
    let r_prev = egui::Rect::from_min_max(rect.min, egui::pos2(rect.min.x + zone_w, rect.max.y));
    let prev_resp = ui.interact(r_prev, ui.id().with("zone_prev"), egui::Sense::click());
    if prev_resp.clicked() {
        // audio_manager.prev(); // Todo: Implement prev
        show_transient_icon("skip-previous-rounded-fill");
    }
    // Icono visible
    let icon_size = zone_w * 0.4;
    if prev_resp.hovered() || rect.contains(ui.input(|i| i.pointer.hover_pos().unwrap_or_default())) {
         ui.put(egui::Rect::from_center_size(r_prev.center(), egui::vec2(icon_size, icon_size)),
            egui::Image::new(egui::include_image!("../../assets/icons/skip-previous-rounded-fill.svg"))
                .tint(egui::Color32::from_white_alpha(200)));
    }

    // Zona Play/Pause/Stop
    let r_play = egui::Rect::from_min_max(egui::pos2(rect.min.x + zone_w, rect.min.y), egui::pos2(rect.min.x + zone_w * 2.0, rect.max.y));
    let play_resp = ui.interact(r_play, ui.id().with("zone_play"), egui::Sense::click());
    if play_resp.clicked() {
        audio_manager.toggle_play_pause();
        let icon = if audio_manager.is_playing() { "pause-rounded-fill" } else { "play-rounded-fill" };
        show_transient_icon(icon);
    }
    if play_resp.long_touched() {
        audio_manager.stop();
        show_transient_icon("stop-rounded-fill");
    }
    // Icono visible Play/Pause
    let icon_size = zone_w * 0.4;
    // let _play_icon_path = format!("../../assets/icons/{}.svg", play_icon); // Unused
    
    // Aquí usamos include_image! si es estático, pero image name es dinámico.
    // Usaremos uri para dinamismo o carga condicional.
    // Para simplificar, cargamos ambos y mostramos uno.
    if play_resp.hovered() || rect.contains(ui.input(|i| i.pointer.hover_pos().unwrap_or_default())) {
         let icon_img = if audio_manager.is_playing() {
             egui::Image::new(egui::include_image!("../../assets/icons/pause-circle-rounded-fill.svg"))
         } else {
             egui::Image::new(egui::include_image!("../../assets/icons/play-circle-rounded-fill.svg"))
         };
         
         ui.put(egui::Rect::from_center_size(r_play.center(), egui::vec2(icon_size, icon_size)),
            icon_img.tint(egui::Color32::from_white_alpha(200)));
    }


    // Zona Siguiente
    let r_next = egui::Rect::from_min_max(egui::pos2(rect.min.x + zone_w * 2.0, rect.min.y), rect.max);
    let next_resp = ui.interact(r_next, ui.id().with("zone_next"), egui::Sense::click());
    if next_resp.clicked() {
        // audio_manager.next(); // Todo: Implement next
        show_transient_icon("skip-next-rounded-fill");
    }
    if next_resp.hovered() || rect.contains(ui.input(|i| i.pointer.hover_pos().unwrap_or_default())) {
         ui.put(egui::Rect::from_center_size(r_next.center(), egui::vec2(icon_size, icon_size)),
            egui::Image::new(egui::include_image!("../../assets/icons/skip-next-rounded-fill.svg"))
                .tint(egui::Color32::from_white_alpha(200)));
    }

    // Dibujar iconos transitorios si es necesario
    draw_transient_icon_ui(ui, rect);
}

static VOLUME_FEEDBACK_TIME: parking_lot::Mutex<f64> = parking_lot::Mutex::new(0.0);
static TRANSIENT_ICON: parking_lot::Mutex<Option<(&'static str, f64)>> = parking_lot::Mutex::new(None);

fn show_transient_icon(icon: &'static str) {
    *TRANSIENT_ICON.lock() = Some((icon, -1.0)); // Se inicializará el tiempo en el siguiente frame
}

fn draw_transient_icon_ui(ui: &mut egui::Ui, rect: egui::Rect) {
    let mut icon_data = TRANSIENT_ICON.lock();
    if let Some((icon_name, ref mut start_time)) = *icon_data {
        let now = ui.input(|i| i.time);
        if *start_time < 0.0 { *start_time = now; }
        
        let elapsed = now - *start_time;
        if elapsed < 1.0 {
            let alpha = ((1.0 - elapsed) * 255.0) as u8;
            let icon_path = format!("../../assets/icons/{}.svg", icon_name);
            let icon_size = rect.width() * 0.2;
            
            ui.put(egui::Rect::from_center_size(rect.center(), egui::vec2(icon_size, icon_size)),
                egui::Image::new(egui::ImageSource::Uri(icon_path.into()))
                    .tint(egui::Color32::from_white_alpha(alpha)));
            ui.ctx().request_repaint();
        } else {
            *icon_data = None;
        }
    }
}

fn draw_volume_feedback(ui: &mut egui::Ui, painter: &egui::Painter, rect: egui::Rect, volume: f32) {
    let now = ui.input(|i| i.time);
    let last_time = VOLUME_FEEDBACK_TIME.lock();
    let elapsed = now - *last_time;
    
    if elapsed < 1.0 {
        let alpha = ((1.0 - elapsed) * 255.0) as u8;
        let color = egui::Color32::from_white_alpha(alpha);
        let center = rect.center();
        
        let icon_size = rect.width() * 0.15;
        ui.put(egui::Rect::from_center_size(center - egui::vec2(0.0, 20.0), egui::vec2(icon_size, icon_size)),
            egui::Image::new(egui::include_image!("../../assets/icons/volume-up-rounded-fill.svg"))
                .tint(color));
        
        painter.text(
            center + egui::vec2(0.0, 30.0),
            egui::Align2::CENTER_CENTER,
            format!("{}", (volume * 100.0) as i32),
            egui::FontId::proportional(20.0),
            color
        );
        ui.ctx().request_repaint();
    }
}
