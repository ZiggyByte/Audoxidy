//! Configuration defaults, serialization and backup rotation.
//!
//! The tests build their own RON documents and write them into a private
//! directory, so the default fallbacks, the serializer round trip, the window
//! geometry decisions and the backup rotation are exercised without reading or
//! writing the user's configuration file.

use super::*;
use crate::utils::test_support::TempTestDir;

/// El texto visible de `ConfigError` no debe derivar: la ruta inválida se
/// conserva byte a byte y E/S/serialización reproducen el mensaje original
/// de su causa subyacente, que además queda expuesta vía `source()`.
#[test]
fn config_error_display_messages_are_stable() {
    use std::error::Error as _;

    assert_eq!(
        ConfigError::InvalidPath.to_string(),
        "ruta de configuración inválida"
    );
    assert!(ConfigError::InvalidPath.source().is_none());

    let io = ConfigError::Io {
        path: PathBuf::from("/tmp/audoxidy.ron"),
        source: std::io::Error::new(std::io::ErrorKind::NotFound, "no encontrado"),
    };
    assert_eq!(io.to_string(), "no encontrado");
    assert_eq!(io.to_string(), io.source().unwrap().to_string());
    assert!(io.source().is_some());

    let serialize = ConfigError::Serialize {
        source: ron::from_str::<AppConfig>("config_version: [")
            .expect_err("RON inválido")
            .into(),
    };
    assert_eq!(
        serialize.to_string(),
        serialize.source().unwrap().to_string()
    );
    assert!(serialize.source().is_some());
}

#[test]
fn test_missing_fields_use_defaults() {
    // Omite `config_version` (campo de nivel superior) y `ui.theme`
    // (campo anidado); ambos deben caer en sus valores por defecto.
    let ron_text = r#"
    (
        profile: Default,
        audio: (
            safety_buffer_secs: 1.0,
        ),
        ui: (
            window_width: 1024.0,
            window_height: 720.0,
        ),
        logging: (
            level: "info",
            max_file_size_mb: 10,
            max_history_files: 5,
            directory: "logs",
        ),
        behavior: (
            auto_save_interval_secs: 20,
            prefetch_seconds_before_end: 30.0,
        ),
    )
    "#;
    let config: AppConfig =
        ron::from_str(ron_text).expect("los campos ausentes deben usar los valores por defecto");
    assert_eq!(config.config_version, 1);
    assert_eq!(config.ui.theme, "dark");
    assert!(
        !config.shortcuts.enabled,
        "la sección de atajos ausente debe quedar deshabilitada"
    );
    assert!(
        config.shortcuts.bindings.is_empty(),
        "la sección de atajos ausente no debe declarar enlaces"
    );
}

#[test]
fn shortcuts_round_trip() {
    // El formato RON documentado para la sección de atajos debe
    // sobrevivir a la serialización y al parseo sin perder enlaces.
    let mut original = AppConfig::default();
    original.shortcuts = ShortcutsConfig {
        enabled: true,
        bindings: vec![ShortcutBinding {
            action: ShortcutAction::Next,
            keys: "Ctrl+Right".into(),
        }],
    };

    let encoded = ron::ser::to_string(&original).expect("serializar la configuración");
    let decoded: AppConfig = ron::from_str(&encoded).expect("releer la configuración");

    assert_eq!(decoded.shortcuts, original.shortcuts);
}

#[test]
fn test_legacy_config_fixture_loads() {
    let config: AppConfig = ron::from_str(include_str!("fixtures/config_legacy.ron"))
        .expect("la configuración heredada debe seguir cargando");
    assert_eq!(config.config_version, 1);
    assert_eq!(config.audio.sample_rate, Some(48000));
    assert_eq!(config.audio.safety_buffer_secs, 2.0);
    assert_eq!(config.logging.level, "info");
    assert_eq!(config.behavior.prefetch_seconds_before_end, 30.0);
}

/// Copias `{nombre}.bak.` existentes para la ruta dada.
fn backups_of(path: &Path) -> Vec<PathBuf> {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    let prefix = format!("{name}.bak.");
    let dir = path
        .parent()
        .expect("la ruta temporal tiene directorio padre");
    let mut found: Vec<PathBuf> = std::fs::read_dir(dir)
        .expect("leer el directorio temporal")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(&prefix))
        })
        .collect();
    found.sort();
    found
}

#[test]
fn test_parse_error_backs_up_and_preserves_original() {
    let dir = TempTestDir::new("config-parse-error");
    let path = dir.path().join("audoxidy_cfg_parse_error.ron");
    let original = b"config_version: [";
    std::fs::write(&path, original).expect("escribir config inválida");

    let config = load_config_from(&path);

    let backups = backups_of(&path);
    assert!(
        !backups.is_empty(),
        "un error de parseo debe crear una copia de seguridad"
    );
    assert!(
        backups
            .iter()
            .any(|b| std::fs::read(b).expect("leer backup") == original),
        "la copia debe conservar los bytes originales"
    );
    assert_eq!(config.config_version, 1);
}

#[test]
fn test_validation_error_backs_up() {
    let dir = TempTestDir::new("config-validation-error");
    let path = dir.path().join("audoxidy_cfg_validation_error.ron");
    let mut invalid = AppConfig::default();
    invalid.audio.safety_buffer_secs = 0.0;
    save_config_to(&path, &invalid).expect("escribir config inválida");
    let original = std::fs::read(&path).expect("leer config inválida");

    let config = load_config_from(&path);

    let backups = backups_of(&path);
    assert!(
        !backups.is_empty(),
        "un fallo de validación debe crear una copia de seguridad"
    );
    assert!(
        backups
            .iter()
            .any(|b| std::fs::read(b).expect("leer backup") == original),
        "la copia debe conservar los bytes originales"
    );
    assert_eq!(config.config_version, 1);
}

#[test]
fn test_backup_rotation_keeps_three() {
    let dir = TempTestDir::new("config-rotation");
    let path = dir.path().join("audoxidy_cfg_rotation.ron");
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .expect("nombre de archivo")
        .to_string();
    for n in 1..=4u64 {
        std::fs::write(path.with_file_name(format!("{name}.bak.{n}")), b"backup")
            .expect("escribir backup falso");
    }

    rotate_config_backups(&path, CONFIG_BACKUP_KEEP).expect("rotar backups");

    let remaining = backups_of(&path);
    assert_eq!(remaining.len(), 3, "solo deben quedar las 3 más recientes");
    assert!(
        !path.with_file_name(format!("{name}.bak.1")).exists(),
        "la copia más antigua debe eliminarse"
    );
    for n in 2..=4u64 {
        assert!(
            path.with_file_name(format!("{name}.bak.{n}")).exists(),
            "la copia {n} debe conservarse"
        );
    }
}

/// A UI config with the geometry fields under the test's control, so the
/// startup decision does not depend on the auto-detected hardware profile.
fn ui_config() -> UiConfig {
    UiConfig {
        window_width: 1360.0,
        window_height: 880.0,
        window_x: None,
        window_y: None,
        window_maximized: false,
        window_geometry_initialized: false,
        window_maximized_width: None,
        window_maximized_height: None,
        theme: "dark".into(),
    }
}

#[test]
fn test_first_launch_starts_maximized() {
    // No geometry has ever been persisted: the window opens maximized.
    let ui = ui_config();
    assert!(ui.starts_maximized());
    assert_eq!(ui.startup_size(), ui.window_size());
    assert_eq!(ui.startup_position(), None);
}

#[test]
fn test_saved_normal_geometry_is_restored() {
    let ui = UiConfig {
        window_geometry_initialized: true,
        window_x: Some(120.0),
        window_y: Some(80.0),
        window_maximized: false,
        ..ui_config()
    };

    assert!(!ui.starts_maximized());
    assert_eq!(ui.startup_size(), (1360.0, 880.0));
    assert_eq!(ui.startup_position(), Some((120.0, 80.0)));
}

#[test]
fn test_saved_maximized_state_restores_full_screen_size() {
    let ui = UiConfig {
        window_geometry_initialized: true,
        window_maximized: true,
        window_maximized_width: Some(2560.0),
        window_maximized_height: Some(1440.0),
        ..ui_config()
    };

    assert!(ui.starts_maximized());
    assert_eq!(ui.startup_size(), (2560.0, 1440.0));
    // A maximized window is centered by the window manager.
    assert_eq!(ui.startup_position(), None);
}

#[test]
fn test_maximized_without_known_screen_size_falls_back_to_normal_size() {
    // A maximized state with no recorded full-screen size still opens
    // maximized, using the saved normal size as the creation hint.
    let ui = UiConfig {
        window_geometry_initialized: true,
        window_maximized: true,
        ..ui_config()
    };

    assert!(ui.starts_maximized());
    assert_eq!(ui.startup_size(), (1360.0, 880.0));
    assert_eq!(ui.startup_position(), None);
}

#[test]
fn test_startup_position_requires_both_coordinates() {
    let only_x = UiConfig {
        window_geometry_initialized: true,
        window_x: Some(500.0),
        ..ui_config()
    };
    assert_eq!(only_x.startup_position(), None);

    let only_y = UiConfig {
        window_geometry_initialized: true,
        window_y: Some(500.0),
        ..ui_config()
    };
    assert_eq!(only_y.startup_position(), None);
}

#[test]
fn test_geometry_fields_survive_ron_round_trip() {
    let original = UiConfig {
        window_x: Some(64.0),
        window_y: Some(32.0),
        window_maximized: true,
        window_geometry_initialized: true,
        window_maximized_width: Some(1920.0),
        window_maximized_height: Some(1080.0),
        ..ui_config()
    };

    let encoded = ron::ser::to_string(&original).expect("serializar la geometría");
    let decoded: UiConfig = ron::from_str(&encoded).expect("releer la geometría");

    assert_eq!(decoded.window_x, Some(64.0));
    assert_eq!(decoded.window_y, Some(32.0));
    assert!(decoded.window_maximized);
    assert!(decoded.window_geometry_initialized);
    assert_eq!(decoded.window_maximized_width, Some(1920.0));
    assert_eq!(decoded.window_maximized_height, Some(1080.0));
}

#[test]
fn test_legacy_config_without_geometry_fields_uses_defaults() {
    // An older config file that predates the window-shell fields must still
    // deserialize, leaving the geometry in its "never persisted" state.
    let ui: UiConfig =
        ron::from_str("(window_width: 1280.0, window_height: 800.0)").expect("config heredada");

    assert_eq!(ui.window_x, None);
    assert_eq!(ui.window_y, None);
    assert!(!ui.window_maximized);
    assert!(!ui.window_geometry_initialized);
    assert_eq!(ui.window_maximized_width, None);
    assert_eq!(ui.window_maximized_height, None);
    // The legacy file predates the persisted geometry, so the first launch
    // after the upgrade opens maximized.
    assert!(ui.starts_maximized());
}
