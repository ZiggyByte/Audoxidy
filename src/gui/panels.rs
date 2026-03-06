use eframe::egui;
use crate::audio::AudioManager;

pub struct PanelManager {
    pub playlist_manager: crate::gui::playlist::PlaylistManager,
    pub filters_manager: crate::gui::library_filters::LibraryFiltersManager,
    pub library_manager: crate::gui::library::LibraryManager,
}

impl PanelManager {
    pub fn new() -> Self {
        Self { 
            playlist_manager: crate::gui::playlist::PlaylistManager::default(),
            filters_manager: crate::gui::library_filters::LibraryFiltersManager::default(),
            library_manager: crate::gui::library::LibraryManager::default(),
        }
    }

    pub fn show(&mut self, 
        ctx: &egui::Context, 
        audio_manager: &AudioManager, 
        audio_center_open: &mut bool,
        database: std::sync::Arc<std::sync::Mutex<crate::db::Database>>,
        scanner: std::sync::Arc<crate::db::scanner::Scanner>
    ) {
        let frame = egui::Frame::window(&ctx.style())
            .inner_margin(0.0)
            .fill(crate::gui::theme::COLOR_BG)
            .shadow(egui::epaint::Shadow::NONE)
            .stroke(egui::Stroke::NONE);

        let screen_rect = ctx.screen_rect();
        let avail_w = screen_rect.width();
        let avail_h = screen_rect.height();

        egui::Window::new("Reproductor")
            .title_bar(false)
            .resizable(false)
            .frame(frame)
            .fixed_size([400.0, 400.0])
            .default_pos([0.0, 0.0])
            .show(ctx, |ui| {
                crate::gui::player::show_player(ui, audio_manager, audio_center_open, &mut self.playlist_manager);
            });

        egui::Window::new("Listas")
            .title_bar(false)
            .resizable(false)
            .frame(frame)
            .fixed_size([400.0, (avail_h - 400.0).max(100.0)])
            .default_pos([0.0, 400.0])
            .show(ctx, |ui| {
                crate::gui::playlist::show_playlist(ui, &mut self.playlist_manager, audio_manager, audio_center_open, &database);
            });

        egui::Window::new("Filtros")
            .title_bar(false)
            .resizable(false)
            .frame(frame)
            .fixed_size([180.0, avail_h.max(100.0)])
            .default_pos([400.0, 0.0])
            .show(ctx, |ui| {
                crate::gui::library_filters::show_library_filters(ui, &mut self.filters_manager, &database);
            });

        egui::Window::new("Biblioteca")
            .title_bar(false)
            .resizable(false)
            .frame(frame)
            .fixed_size([(avail_w - 580.0).max(300.0), avail_h.max(100.0)])
            .default_pos([580.0, 0.0])
            .show(ctx, |ui| {
                crate::gui::library::show_library(ui, &mut self.library_manager, &database, &scanner, &mut self.playlist_manager, audio_manager);
            });
    }
}
