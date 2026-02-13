mod audio;
mod gui;
mod library;
mod integrations;
mod utils;

use eframe::egui;

use crate::audio::AudioManager;

fn main() -> eframe::Result {
    tracing_subscriber::fmt::init();
    
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([400.0, 300.0])
            .with_transparent(true)
            .with_decorations(false),
        ..Default::default()
    };

    eframe::run_native(
        "AuDoxiDY",
        native_options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            crate::gui::setup_theme(&cc.egui_ctx);
            let audio_manager = std::sync::Arc::new(AudioManager::new().expect("No se pudo inicializar el motor de audio"));
            Ok(Box::new(AudoxidyApp::new(cc, audio_manager)))
        }),
    )
}

use std::sync::Arc;
use crate::library::db::MediaLibrary;
use crate::library::scanner::LibraryScanner;
use crate::gui::audio_center::AudioCenter;

struct AudoxidyApp {
    audio_manager: Arc<AudioManager>,
    library: Arc<MediaLibrary>,
    scanner: LibraryScanner,
    panel_manager: crate::gui::panels::PanelManager,
    audio_center: AudioCenter,
    media_controls: crate::integrations::media_controls::SystemMediaControls,
}

impl AudoxidyApp {
    fn new(_cc: &eframe::CreationContext<'_>, audio_manager: Arc<AudioManager>) -> Self {
        let library_path = "library.db"; // Podríamos usar directorios específicos de usuario luego
        let library = Arc::new(MediaLibrary::new(library_path).expect("No se pudo inicializar la biblioteca"));
        let scanner = LibraryScanner::new(library.clone());

        let media_controls = crate::integrations::media_controls::SystemMediaControls::new(audio_manager.clone())
            .expect("Error al inicializar controles multimedia del sistema");

        Self { 
            audio_manager,
            library,
            scanner,
            panel_manager: crate::gui::panels::PanelManager::new(),
            audio_center: AudioCenter::default(),
            media_controls,
        }
    }
}

impl eframe::App for AudoxidyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Obtener estado actual para actualizar controles
        let state = self.audio_manager.get_state();
        self.media_controls.update(&state);

        egui::CentralPanel::default().show(ctx, |_ui| {
            self.panel_manager.show(ctx, &self.audio_manager, self.library.clone(), &self.scanner, &mut self.audio_center.open);
        });
        
        self.audio_center.show(ctx, &self.audio_manager);
        
        // Solicitar repintado continuo para animaciones si es necesario
        // ctx.request_repaint();
    }
}
