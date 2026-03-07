mod audio;
mod gui;
mod db;
mod integrations;
mod utils;

use crate::audio::AudioManager;
use crate::gui::app::AudoxidyApp;

fn main() -> iced::Result {
    tracing_subscriber::fmt::init();

    let audio_manager = std::sync::Arc::new(
        AudioManager::new().expect("No se pudo inicializar el motor de audio")
    );

    iced::application(
        move || AudoxidyApp::new(audio_manager.clone()),
        AudoxidyApp::update,
        AudoxidyApp::view,
    )
    .font(include_bytes!("../assets/fonts/Inter.ttf").as_slice())
    .font(include_bytes!("../assets/fonts/Stage-Wanders.ttf").as_slice())
    .title(|_state: &AudoxidyApp| String::from("AuDoxiDY"))
    .subscription(|state: &AudoxidyApp| state.subscription())
    .theme(|state: &AudoxidyApp| state.theme())
    .window(iced::window::Settings {
        size: iced::Size::new(1400.0, 900.0),
        min_size: Some(iced::Size::new(400.0, 300.0)),
        decorations: false,
        transparent: true,
        ..Default::default()
    })
    .run()
}
