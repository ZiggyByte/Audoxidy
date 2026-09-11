use audoxidy::audio::AudioManager;
use audoxidy::gui::app::AudoxidyApp;

// ── Entry point ──────────────────────────────────────────────

fn main() -> iced::Result {
    // 1. Cargar configuración (crea archivo por defecto si no existe)
    let app_config = audoxidy::utils::config::load_config();

    // 2. Inicializar logging con configuración
    audoxidy::utils::observability::init_logging(&app_config.logging);

    tracing::info!(
        "Audoxidy {} iniciando con perfil {:?}",
        env!("CARGO_PKG_VERSION"),
        app_config.profile
    );

    // 3. Inicializar gestor de memoria
    audoxidy::utils::memory_manager::MemoryManager::init();

    // Vigilante de deadlocks: solo en builds de desarrollo con la feature activa.
    #[cfg(feature = "deadlock-detection")]
    audoxidy::utils::observability::spawn_deadlock_watchdog();

    // 4. Inicializar motor de audio
    let audio_manager =
        std::sync::Arc::new(AudioManager::new().expect("No se pudo inicializar el motor de audio"));

    let window_size = app_config.ui.window_size();

    tracing::info!(
        "Audoxidy listo. Ventana: {}x{}, Audio perfil: {:?}",
        window_size.0,
        window_size.1,
        app_config.profile
    );

    // 5. Lanzar aplicación Iced
    iced::application(
        move || AudoxidyApp::new(audio_manager.clone()),
        AudoxidyApp::update,
        AudoxidyApp::view,
    )
    .font(include_bytes!("../assets/fonts/Inter.ttf").as_slice())
    .font(include_bytes!("../assets/fonts/Stage-Wanders.ttf").as_slice())
    .title(|_state: &AudoxidyApp| String::from("Audoxidy"))
    .subscription(|state: &AudoxidyApp| state.subscription())
    .theme(|state: &AudoxidyApp| state.theme())
    .window(iced::window::Settings {
        size: iced::Size::new(window_size.0, window_size.1),
        min_size: Some(iced::Size::new(900.0, 500.0)),
        decorations: false,
        transparent: true,
        ..Default::default()
    })
    .run()
}
