use eframe::egui;
use egui_dock::{DockArea, DockState, NodeIndex, Style, TabViewer};
use crate::audio::AudioManager;

use std::sync::Arc;
use crate::library::db::MediaLibrary;
use crate::library::scanner::LibraryScanner;

pub struct Tab {
    pub name: String,
    pub module_type: ModuleType,
}

pub enum ModuleType {
    Player,
    Playlists,
    LibraryFilters,
    Library,
}

pub struct AudoxidyTabViewer<'a> {
    pub audio_manager: &'a AudioManager,
    pub library: Arc<MediaLibrary>,
    pub scanner: &'a LibraryScanner,
    pub audio_center_open: &'a mut bool,
}

impl<'a> TabViewer for AudoxidyTabViewer<'a> {
    type Tab = Tab;

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        tab.name.clone().into()
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        match tab.module_type {
            ModuleType::Player => crate::gui::player::show_player(ui, self.audio_manager, self.audio_center_open),
            ModuleType::Playlists => { ui.label("Listas de Reproducción"); },
            ModuleType::LibraryFilters => { 
                crate::gui::library::show_library_filters(ui, self.library.clone());
            },
            ModuleType::Library => { 
                crate::gui::library::show_library(ui, self.audio_manager, self.library.clone(), self.scanner);
            },
        }
    }
}

pub struct PanelManager {
    pub dock_state: DockState<Tab>,
}

impl PanelManager {
    pub fn new() -> Self {
        let mut dock_state = DockState::new(vec![
            Tab { name: "Reproductor".to_string(), module_type: ModuleType::Player },
        ]);

        let [_, library_id] = dock_state.main_surface_mut().split_right(
            NodeIndex::root(),
            0.3,
            vec![Tab { name: "Biblioteca".to_string(), module_type: ModuleType::Library }],
        );

        dock_state.main_surface_mut().split_left(
            library_id,
            0.2, // Panel de filtros a la izquierda de la biblioteca
            vec![Tab { name: "Filtros".to_string(), module_type: ModuleType::LibraryFilters }],
        );

        dock_state.main_surface_mut().split_below(
            library_id,
            0.5,
            vec![Tab { name: "Listas".to_string(), module_type: ModuleType::Playlists }],
        );

        Self { dock_state }
    }

    pub fn show(&mut self, ctx: &egui::Context, audio_manager: &AudioManager, library: Arc<MediaLibrary>, scanner: &LibraryScanner, audio_center_open: &mut bool) {
        let mut style = Style::from_egui(ctx.style().as_ref());
        style.tab.active.bg_fill = crate::gui::theme::COLOR_BG;
        style.tab.active.outline_color = egui::Color32::TRANSPARENT;
        style.tab.active.corner_radius = egui::CornerRadius::ZERO;
        
        DockArea::new(&mut self.dock_state)
            .style(style)
            .show(ctx, &mut AudoxidyTabViewer { 
                audio_manager,
                library,
                scanner,
                audio_center_open,
            });
    }
}
