use audoxidy::core::{Core, CorePaths};
use audoxidy::gui::app::AudoxidyApp;
use std::sync::{Arc, Mutex};

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
    let audio_manager = Arc::new(
        audoxidy::audio::AudioManager::new().expect("No se pudo inicializar el motor de audio"),
    );

    // 5. Inicializar base de datos principal y base de datos del escáner.
    //    El escáner usa su propia conexión para no compartir el lock con la GUI.
    let database = Arc::new(Mutex::new(
        audoxidy::db::Database::new().expect("Error crítico al crear/iniciar la base de datos."),
    ));
    audio_manager.set_database(database.clone());
    let scanner_database = Arc::new(Mutex::new(
        audoxidy::db::Database::new().expect("Error al inicializar la base de datos del escáner."),
    ));
    let scanner = Arc::new(audoxidy::db::scanner::Scanner::new(scanner_database));

    // 6. Construir el dominio Core con sus rutas inyectadas.
    let core = Arc::new(Core::new(
        audio_manager,
        database,
        scanner,
        CorePaths::desktop_default(),
    ));

    let window_size = app_config.ui.window_size();

    tracing::info!(
        "Audoxidy listo. Ventana: {}x{}, Audio perfil: {:?}",
        window_size.0,
        window_size.1,
        app_config.profile
    );

    // 7. Lanzar aplicación Iced
    iced::application(
        move || AudoxidyApp::new(core.clone()),
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
