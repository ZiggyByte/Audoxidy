use audoxidy::core::{Core, CorePaths};
use audoxidy::gui::app::AudoxidyApp;
use audoxidy::utils::paths::AppPaths;
use std::sync::{Arc, Mutex};

// ── Entry point ──────────────────────────────────────────────

fn main() -> iced::Result {
    // 1. Resolve the Core filesystem paths and publish them crate-wide.
    let core_paths = CorePaths::desktop_default();
    audoxidy::utils::paths::init(AppPaths {
        db_path: core_paths.db_path.clone(),
        cover_cache_dir: core_paths.cover_cache_dir.clone(),
        config_path: core_paths.config_path.clone(),
    });

    // 2. Cargar configuración (crea archivo por defecto si no existe)
    let app_config = audoxidy::utils::config::load_config_from(&core_paths.config_path);

    // 3. Inicializar logging con configuración
    audoxidy::utils::observability::init_logging(&app_config.logging);

    tracing::info!(
        "Audoxidy {} iniciando con perfil {:?}",
        env!("CARGO_PKG_VERSION"),
        app_config.profile
    );

    // 4. Inicializar gestor de memoria
    audoxidy::utils::memory_manager::MemoryManager::init();

    // Vigilante de deadlocks: solo en builds de desarrollo con la feature activa.
    #[cfg(feature = "deadlock-detection")]
    audoxidy::utils::observability::spawn_deadlock_watchdog();

    // 5. Inicializar motor de audio
    let audio_manager = Arc::new(
        audoxidy::audio::AudioManager::new().expect("No se pudo inicializar el motor de audio"),
    );

    // 6. Inicializar base de datos principal y base de datos del escáner.
    //    El escáner usa su propia conexión para no compartir el lock con la GUI.
    let database = Arc::new(Mutex::new(
        audoxidy::db::Database::new_at(&core_paths.db_path)
            .expect("Error crítico al crear/iniciar la base de datos."),
    ));
    audio_manager.set_database(database.clone());
    let scanner_database = Arc::new(Mutex::new(
        audoxidy::db::Database::new_at(&core_paths.db_path)
            .expect("Error al inicializar la base de datos del escáner."),
    ));
    let scanner = Arc::new(audoxidy::db::scanner::Scanner::new(scanner_database));

    // 7. Construir el dominio Core con sus rutas inyectadas.
    let core = Arc::new(Core::new(audio_manager, database, scanner, core_paths));

    // Initial geometry: a fresh install opens maximized; later launches
    // restore the saved size, position and maximized state. Applying it here,
    // while the window is created, avoids the flash of opening at one size and
    // maximizing moments later.
    let ui = &app_config.ui;
    let window_maximized = ui.starts_maximized();
    let window_size = ui.startup_size();
    let window_position = match ui.startup_position() {
        Some((x, y)) => iced::window::Position::Specific(iced::Point::new(x, y)),
        None => iced::window::Position::Centered,
    };

    tracing::info!(
        "Audoxidy listo. Ventana: {}x{}, Audio perfil: {:?}",
        window_size.0,
        window_size.1,
        app_config.profile
    );

    // 8. Lanzar aplicación Iced
    iced::application(
        move || AudoxidyApp::new(core.clone()),
        AudoxidyApp::update,
        AudoxidyApp::view,
    )
    .font(include_bytes!("assets/fonts/Inter.ttf").as_slice())
    .font(include_bytes!("assets/fonts/Stage-Wanders.ttf").as_slice())
    .title(|_state: &AudoxidyApp| String::from("Audoxidy"))
    .subscription(|state: &AudoxidyApp| state.subscription())
    .theme(|state: &AudoxidyApp| state.theme())
    .window(iced::window::Settings {
        size: iced::Size::new(window_size.0, window_size.1),
        position: window_position,
        maximized: window_maximized,
        min_size: Some(iced::Size::new(900.0, 500.0)),
        decorations: false,
        transparent: true,
        ..Default::default()
    })
    .run()
}
