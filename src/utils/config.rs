//! Sistema de configuración unificado de Audoxidy.
//!
//! Centraliza toda la configuración de la aplicación en un solo struct (`AppConfig`)
//! con serialización RON, validación al inicio, perfiles predefinidos y recarga
//! en caliente mediante detección de cambios en el archivo de configuración.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

// ── Perfiles de configuración ─────────────────────────────────

/// Perfiles predefinidos de configuración.
/// Cada perfil ajusta automáticamente múltiples parámetros para el caso de uso.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfigProfile {
    /// Máxima fidelidad: sin concesiones en calidad de audio.
    /// Sample rate nativo, upsample automático, buffer grande.
    HighFidelity,
    /// Balance entre calidad y recursos.
    Default,
    /// Mínimo consumo de RAM/CPU. Ideal para equipos con <8GB RAM o <4 núcleos.
    LowResource,
}

impl Default for ConfigProfile {
    fn default() -> Self {
        // Detectar automáticamente basado en hardware
        let sys = sysinfo::System::new_all();
        let ram_gb = sys.total_memory() / (1024 * 1024 * 1024);
        let cores = sys.cpus().len();
        if ram_gb < 8 || cores < 4 {
            ConfigProfile::LowResource
        } else if ram_gb >= 32 && cores >= 8 {
            ConfigProfile::HighFidelity
        } else {
            ConfigProfile::Default
        }
    }
}

// ── Estructura principal de configuración ─────────────────────

/// Configuración completa de Audoxidy.
/// Se serializa como archivo RON en `~/.config/audoxidy/config.ron`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    // Metadatos del archivo
    pub config_version: u32,

    // Perfil activo
    pub profile: ConfigProfile,

    // Audio
    pub audio: AudioConfig,

    // Interfaz de usuario
    pub ui: UiConfig,

    // Logging
    pub logging: LoggingConfig,

    // Comportamiento
    pub behavior: BehaviorConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        let profile = ConfigProfile::default();
        Self {
            config_version: 1,
            profile,
            audio: AudioConfig::for_profile(profile),
            ui: UiConfig::for_profile(profile),
            logging: LoggingConfig::default(),
            behavior: BehaviorConfig::default(),
        }
    }
}

// ── Subconfiguraciones ────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    /// Host de audio (ALSA, PipeWire, WASAPI, CoreAudio, etc.)
    pub host: Option<String>,
    /// Dispositivo de salida
    pub device: Option<String>,
    /// Sample rate en Hz (None = auto)
    pub sample_rate: Option<u32>,
    /// Profundidad de bits
    pub bit_depth: Option<BitDepthConfig>,
    /// Canales (None = auto)
    pub channels: Option<u16>,
    /// Tamaño de buffer en frames (None = auto)
    pub buffer_size: Option<u32>,
    /// Buffer de seguridad (segundos de audio precargado)
    pub safety_buffer_secs: f64,
}

impl AudioConfig {
    fn for_profile(profile: ConfigProfile) -> Self {
        match profile {
            ConfigProfile::HighFidelity => Self {
                host: None,
                device: None,
                sample_rate: None,
                bit_depth: Some(BitDepthConfig::Bits32Float),
                channels: None,
                buffer_size: None,
                safety_buffer_secs: 2.0,
            },
            ConfigProfile::Default => Self {
                host: None,
                device: None,
                sample_rate: None,
                bit_depth: None,
                channels: None,
                buffer_size: None,
                safety_buffer_secs: 2.0,
            },
            ConfigProfile::LowResource => Self {
                host: None,
                device: None,
                sample_rate: Some(44100),
                bit_depth: Some(BitDepthConfig::Bits16),
                channels: Some(2),
                buffer_size: Some(512),
                safety_buffer_secs: 0.5,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BitDepthConfig {
    Bits16,
    Bits24,
    Bits32Float,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    /// Ancho de ventana
    pub window_width: f32,
    /// Alto de ventana
    pub window_height: f32,
    /// Tema (dark/light)
    pub theme: String,
}

impl UiConfig {
    /// Devuelve el tamaño de ventana como tupla (ancho, alto).
    pub fn window_size(&self) -> (f32, f32) {
        (self.window_width, self.window_height)
    }
}

impl UiConfig {
    fn for_profile(profile: ConfigProfile) -> Self {
        match profile {
            ConfigProfile::HighFidelity => Self {
                window_width: 1600.0,
                window_height: 1000.0,
                theme: "dark".into(),
            },
            ConfigProfile::Default => Self {
                window_width: 1360.0,
                window_height: 880.0,
                theme: "dark".into(),
            },
            ConfigProfile::LowResource => Self {
                window_width: 1024.0,
                window_height: 720.0,
                theme: "dark".into(),
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Nivel global (trace, debug, info, warn, error)
    pub level: String,
    /// Rotación: tamaño máximo por archivo en MB
    pub max_file_size_mb: u64,
    /// Rotación: número máximo de archivos históricos
    pub max_history_files: u32,
    /// Directorio de logs
    pub directory: String,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".into(),
            max_file_size_mb: 10,
            max_history_files: 5,
            directory: "logs".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorConfig {
    /// Intervalo de autoguardado de estado (segundos)
    pub auto_save_interval_secs: u64,
    /// Prefetch de siguiente canción (segundos antes del final)
    pub prefetch_seconds_before_end: f64,
}

impl Default for BehaviorConfig {
    fn default() -> Self {
        Self {
            auto_save_interval_secs: 20,
            prefetch_seconds_before_end: 30.0,
        }
    }
}

// ── Validación ────────────────────────────────────────────────

/// Error de validación de configuración.
#[derive(Debug)]
pub struct ConfigValidation {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl ConfigValidation {
    /// Devuelve `true` si no hay errores de validación.
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }
}

/// Valida la configuración y devuelve errores y advertencias.
pub fn validate_config(config: &AppConfig) -> ConfigValidation {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    // Validar sample rate
    if let Some(sr) = config.audio.sample_rate {
        if sr < 8000 || sr > 768000 {
            errors.push(format!("Sample rate {} Hz fuera de rango (8000-768000)", sr));
        } else if ![8000, 11025, 16000, 22050, 44100, 48000, 88200, 96000, 176400, 192000, 352800, 384000, 705600, 768000].contains(&sr) {
            warnings.push(format!("Sample rate {} Hz no es estándar, puede no ser soportado", sr));
        }
    }

    // Validar buffer size
    if let Some(bs) = config.audio.buffer_size {
        if bs < 16 {
            errors.push(format!("Buffer size {} es demasiado pequeño (mínimo 16)", bs));
        } else if bs > 16384 {
            warnings.push(format!("Buffer size {} es muy grande, puede aumentar latencia", bs));
        }
    }

    // Validar safety buffer
    if config.audio.safety_buffer_secs < 0.1 {
        errors.push(format!("Safety buffer {:.1}s es demasiado pequeño (mínimo 0.1s)", config.audio.safety_buffer_secs));
    }

    // Validar logging
    match config.logging.level.as_str() {
        "trace" | "debug" | "info" | "warn" | "error" => {}
        other => warnings.push(format!("Nivel de log '{}' no es estándar, usando info", other)),
    }

    if config.logging.max_file_size_mb == 0 {
        errors.push("max_file_size_mb debe ser > 0".into());
    }

    ConfigValidation { errors, warnings }
}

// ── Persistencia ──────────────────────────────────────────────

/// Ruta por defecto del archivo de configuración.
pub fn default_config_path() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".config/audoxidy/config.ron")
}

/// Carga la configuración desde disco.
/// Si el archivo no existe, crea uno con valores por defecto.
pub fn load_config() -> AppConfig {
    let path = default_config_path();
    if path.exists() {
        match std::fs::read_to_string(&path) {
            Ok(content) => match ron::from_str(&content) {
                Ok(config) => {
                    let validation = validate_config(&config);
                    for w in &validation.warnings {
                        tracing::warn!("Config warning: {}", w);
                    }
                    if !validation.is_valid() {
                        for e in &validation.errors {
                            tracing::error!("Config error: {}", e);
                        }
                        tracing::warn!("Usando configuración por defecto");
                        let default = AppConfig::default();
                        let _ = save_config(&default);
                        return default;
                    }
                    // Aplicar perfil
                    apply_profile(&config);
                    config
                }
                Err(e) => {
                    tracing::error!("Error parseando config: {}. Usando defaults", e);
                    let default = AppConfig::default();
                    let _ = save_config(&default);
                    default
                }
            },
            Err(e) => {
                tracing::warn!("No se pudo leer config: {}. Usando defaults", e);
                AppConfig::default()
            }
        }
    } else {
        tracing::info!("No hay archivo de configuración. Creando {} con valores por defecto.", path.display());
        let config = AppConfig::default();
        let _ = save_config(&config);
        config
    }
}

/// Guarda la configuración a disco.
pub fn save_config(config: &AppConfig) -> Result<(), String> {
    let path = default_config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let content = ron::ser::to_string_pretty(config, ron::ser::PrettyConfig::default())
        .map_err(|e| e.to_string())?;
    std::fs::write(&path, content).map_err(|e| e.to_string())?;
    Ok(())
}

/// Marca de tiempo del archivo de configuración en disco.
static CONFIG_MTIME: std::sync::Mutex<Option<SystemTime>> = std::sync::Mutex::new(None);

/// Verifica si el archivo de configuración cambió en disco y recarga si es necesario.
/// Debe llamarse periódicamente (ej: desde el Tick del loop principal).
pub fn check_config_reload() -> Option<AppConfig> {
    let path = default_config_path();
    if !path.exists() {
        return None;
    }
    let metadata = match std::fs::metadata(&path) {
        Ok(m) => m,
        Err(_) => return None,
    };
    let modified = match metadata.modified() {
        Ok(t) => t,
        Err(_) => return None,
    };

    let mut last_mtime = CONFIG_MTIME.lock().unwrap();
    if let Some(last) = *last_mtime {
        if modified <= last {
            return None; // No cambió
        }
    }
    *last_mtime = Some(modified);

    // Recargar
    match std::fs::read_to_string(&path) {
        Ok(content) => match ron::from_str(&content) {
            Ok(config) => {
                tracing::info!("Configuración recargada desde disco");
                apply_profile(&config);
                Some(config)
            }
            Err(e) => {
                tracing::error!("Error recargando config: {}", e);
                None
            }
        },
        Err(e) => {
            tracing::error!("Error leyendo config para recarga: {}", e);
            None
        }
    }
}

fn apply_profile(config: &AppConfig) {
    match config.profile {
        ConfigProfile::LowResource => {
            crate::utils::LOW_RESOURCE_MODE.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        _ => {
            crate::utils::LOW_RESOURCE_MODE.store(false, std::sync::atomic::Ordering::Relaxed);
        }
    }
}
