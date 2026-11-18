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
///
/// La sección `shortcuts` documenta el formato previsto para enlazar acciones
/// a combinaciones de teclas; puede añadirse al archivo sin tocar código:
///
/// ```ron
/// shortcuts: (
///     enabled: false,
///     bindings: [
///         (action: PlayPause, keys: "Space"),
///         (action: Next, keys: "Ctrl+Right"),
///         (action: Previous, keys: "Ctrl+Left"),
///     ],
/// ),
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
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

    // Atajos de teclado globales
    pub shortcuts: ShortcutsConfig,
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
            shortcuts: ShortcutsConfig::default(),
        }
    }
}

// ── Subconfiguraciones ────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
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
            // Un perfil de bajos recursos es de memoria y CPU, no de fidelidad:
            // la frecuencia de muestreo, el número de canales y la profundidad
            // de bits siguen a lo que negocien el archivo y el dispositivo,
            // exactamente igual que en `Default`. Forzar 48 kHz / 16 bits /
            // estéreo aquí truncaría un archivo de alta resolución justo en los
            // equipos que menos pueden permitírselo, y contradice el valor
            // central del reproductor.
            //
            // El tamaño del búfer también lo elige el dispositivo: fijar 512
            // frames provoca underruns a frecuencias altas, y la memoria que
            // ahorra este perfil viene de la duración del ring buffer en
            // `AudioEngine::new`, no de aquí.
            ConfigProfile::LowResource => Self {
                host: None,
                device: None,
                sample_rate: None,
                bit_depth: None,
                channels: None,
                buffer_size: None,
                safety_buffer_secs: 0.5,
            },
        }
    }
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self::for_profile(ConfigProfile::default())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BitDepthConfig {
    Bits16,
    Bits24,
    Bits32Float,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    /// Ancho de ventana
    pub window_width: f32,
    /// Alto de ventana
    pub window_height: f32,
    /// Posición horizontal de la ventana (None = centrada)
    pub window_x: Option<f32>,
    /// Posición vertical de la ventana (None = centrada)
    pub window_y: Option<f32>,
    /// Ventana maximizada al iniciar
    pub window_maximized: bool,
    /// Indica si la geometría ya se guardó alguna vez. Distingue el primer
    /// arranque (ventana maximizada) de un arranque con geometría conocida.
    pub window_geometry_initialized: bool,
    /// Último tamaño de pantalla completa conocido. Se usa para crear la
    /// ventana ya maximizada al arrancar, sin parpadeo.
    pub window_maximized_width: Option<f32>,
    pub window_maximized_height: Option<f32>,
    /// Tema (dark/light)
    pub theme: String,
}

impl UiConfig {
    /// Devuelve el tamaño de ventana como tupla (ancho, alto).
    pub fn window_size(&self) -> (f32, f32) {
        (self.window_width, self.window_height)
    }

    /// Whether the window must be created maximized.
    ///
    /// A fresh install (geometry never persisted) and a window that was
    /// maximized when it was last closed both start maximized.
    pub fn starts_maximized(&self) -> bool {
        !self.window_geometry_initialized || self.window_maximized
    }

    /// Size the window is created with.
    ///
    /// When starting maximized, the last known full-screen size is used so the
    /// window manager does not resize it after creation; otherwise the saved
    /// normal size is restored. The known maximized size can be absent on a
    /// first launch, in which case the normal size is the best hint available.
    pub fn startup_size(&self) -> (f32, f32) {
        if self.starts_maximized() {
            match (self.window_maximized_width, self.window_maximized_height) {
                (Some(width), Some(height)) => (width, height),
                _ => self.window_size(),
            }
        } else {
            self.window_size()
        }
    }

    /// Saved top-left position to restore, if any.
    ///
    /// Returns `None` when the window starts maximized or when only one of the
    /// two coordinates was persisted; both are required to place the window.
    pub fn startup_position(&self) -> Option<(f32, f32)> {
        if self.starts_maximized() {
            return None;
        }
        match (self.window_x, self.window_y) {
            (Some(x), Some(y)) => Some((x, y)),
            _ => None,
        }
    }
}

impl UiConfig {
    fn for_profile(profile: ConfigProfile) -> Self {
        match profile {
            ConfigProfile::HighFidelity => Self {
                window_width: 1600.0,
                window_height: 1000.0,
                window_x: None,
                window_y: None,
                window_maximized: false,
                window_geometry_initialized: false,
                window_maximized_width: None,
                window_maximized_height: None,
                theme: "dark".into(),
            },
            ConfigProfile::Default => Self {
                window_width: 1360.0,
                window_height: 880.0,
                window_x: None,
                window_y: None,
                window_maximized: false,
                window_geometry_initialized: false,
                window_maximized_width: None,
                window_maximized_height: None,
                theme: "dark".into(),
            },
            ConfigProfile::LowResource => Self {
                window_width: 1024.0,
                window_height: 720.0,
                window_x: None,
                window_y: None,
                window_maximized: false,
                window_geometry_initialized: false,
                window_maximized_width: None,
                window_maximized_height: None,
                theme: "dark".into(),
            },
        }
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self::for_profile(ConfigProfile::default())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
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
#[serde(default)]
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

// ── Atajos de teclado ─────────────────────────────────────────

/// Acción de reproducción que un atajo de teclado puede desencadenar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShortcutAction {
    /// Alternar entre reproducir y pausar.
    PlayPause,
    /// Detener la reproducción.
    Stop,
    /// Saltar a la pista siguiente.
    Next,
    /// Volver a la pista anterior.
    Previous,
    /// Avanzar la posición de reproducción.
    SeekForward,
    /// Retroceder la posición de reproducción.
    SeekBackward,
    /// Subir el volumen.
    VolumeUp,
    /// Bajar el volumen.
    VolumeDown,
    /// Silenciar o reactivar el audio.
    Mute,
}

/// Enlace entre una [`ShortcutAction`] y la combinación de teclas que la
/// dispara, por ejemplo `"Ctrl+Right"`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShortcutBinding {
    /// Acción de reproducción que se ejecuta al pulsar la combinación.
    pub action: ShortcutAction,
    /// Combinación de teclas en texto, por ejemplo `"Ctrl+Right"`.
    pub keys: String,
}

/// Configuración de atajos de teclado globales.
///
/// El flag `enabled` y los enlaces son únicamente datos persistidos: en esta
/// fase no se registra ningún listener, por lo que activarlo todavía no tiene
/// efecto. La sección existe para que las combinaciones puedan declararse en
/// `config.ron` sin cambios de código cuando se añada el listener.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShortcutsConfig {
    /// Habilita el sistema de atajos globales (aún sin listener registrado).
    pub enabled: bool,
    /// Enlaces previstos: acción → combinación de teclas.
    pub bindings: Vec<ShortcutBinding>,
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
        if !(8000..=768000).contains(&sr) {
            errors.push(format!(
                "Sample rate {} Hz fuera de rango (8000-768000)",
                sr
            ));
        } else if ![
            8000, 11025, 16000, 22050, 44100, 48000, 88200, 96000, 176400, 192000, 352800, 384000,
            705600, 768000,
        ]
        .contains(&sr)
        {
            warnings.push(format!(
                "Sample rate {} Hz no es estándar, puede no ser soportado",
                sr
            ));
        }
    }

    // Validar buffer size
    if let Some(bs) = config.audio.buffer_size {
        if bs < 16 {
            errors.push(format!(
                "Buffer size {} es demasiado pequeño (mínimo 16)",
                bs
            ));
        } else if bs > 16384 {
            warnings.push(format!(
                "Buffer size {} es muy grande, puede aumentar latencia",
                bs
            ));
        }
    }

    // Validar safety buffer
    if config.audio.safety_buffer_secs < 0.1 {
        errors.push(format!(
            "Safety buffer {:.1}s es demasiado pequeño (mínimo 0.1s)",
            config.audio.safety_buffer_secs
        ));
    }

    // Validar logging
    match config.logging.level.as_str() {
        "trace" | "debug" | "info" | "warn" | "error" => {}
        other => warnings.push(format!(
            "Nivel de log '{}' no es estándar, usando info",
            other
        )),
    }

    if config.logging.max_file_size_mb == 0 {
        errors.push("max_file_size_mb debe ser > 0".into());
    }

    ConfigValidation { errors, warnings }
}

// ── Persistencia ──────────────────────────────────────────────

/// Ruta por defecto del archivo de configuración.
pub fn default_config_path() -> PathBuf {
    crate::utils::paths::get().config_path.clone()
}

/// Error al leer o escribir el archivo de configuración.
///
/// Envuelve los fallos de E/S y de serialización RON que antes se devolvían
/// como `String`, reproduciendo exactamente el texto visible original y
/// exponiendo la causa subyacente a través de `source()`. La ruta queda fuera
/// del mensaje porque los sitios que registran el error ya la imprimen.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// La ruta no tiene un nombre de archivo utilizable.
    #[error("ruta de configuración inválida")]
    InvalidPath,
    /// Falló una operación de E/S sobre la ruta indicada.
    #[error("{source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// Falló la serialización de la configuración a RON.
    #[error("{source}")]
    Serialize {
        #[source]
        source: ron::Error,
    },
}

/// Número máximo de copias de seguridad de la configuración que se conservan.
const CONFIG_BACKUP_KEEP: usize = 3;

/// Copia `path` a `{nombre}.bak.{epoch}` en el mismo directorio y recorta las
/// copias antiguas, conservando solo las más recientes.
fn backup_config_file(path: &Path, keep: usize) -> Result<(), ConfigError> {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or(ConfigError::InvalidPath)?;
    let stamp = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let backup = path.with_file_name(format!("{name}.bak.{stamp}"));
    std::fs::copy(path, &backup).map_err(|source| ConfigError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    rotate_config_backups(path, keep)
}

/// Elimina las copias `{nombre}.bak.{n}` más antiguas, conservando las `keep`
/// más recientes.
fn rotate_config_backups(path: &Path, keep: usize) -> Result<(), ConfigError> {
    let (Some(dir), Some(name)) = (path.parent(), path.file_name().and_then(|n| n.to_str())) else {
        return Ok(());
    };
    let prefix = format!("{name}.bak.");
    let mut backups: Vec<(u64, PathBuf)> = std::fs::read_dir(dir)
        .map_err(|source| ConfigError::Io {
            path: dir.to_path_buf(),
            source,
        })?
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let fname = e.file_name();
            let suffix = fname.to_str()?.strip_prefix(&prefix)?;
            Some((suffix.parse::<u64>().ok()?, e.path()))
        })
        .collect();
    backups.sort_by_key(|(stamp, _)| *stamp);
    while backups.len() > keep {
        let (_, oldest) = backups.remove(0);
        if let Err(e) = std::fs::remove_file(&oldest) {
            tracing::warn!(
                "No se pudo borrar backup antiguo {}: {}",
                oldest.display(),
                e
            );
        }
    }
    Ok(())
}

/// Carga la configuración desde la ruta indicada.
/// Si el archivo no existe, crea uno con valores por defecto.
pub fn load_config_from(path: &Path) -> AppConfig {
    if !path.exists() {
        tracing::info!(
            "No hay archivo de configuración. Creando {} con valores por defecto.",
            path.display()
        );
        let config = AppConfig::default();
        if let Err(e) = save_config_to(path, &config) {
            tracing::error!(
                "No se pudo escribir la configuración en {}: {}",
                path.display(),
                e
            );
        }
        return config;
    }

    match std::fs::read_to_string(path) {
        Ok(content) => match ron::from_str::<AppConfig>(&content) {
            Ok(config) => {
                let validation = validate_config(&config);
                for w in &validation.warnings {
                    tracing::warn!("Config warning: {}", w);
                }
                if validation.is_valid() {
                    resolve_resource_profile(&config);
                    config
                } else {
                    for e in &validation.errors {
                        tracing::error!("Config error: {}", e);
                    }
                    tracing::warn!("Usando configuración por defecto");
                    let default = AppConfig::default();
                    if let Err(e) = backup_config_file(path, CONFIG_BACKUP_KEEP) {
                        tracing::error!(
                            "Backup de config falló, no se sobrescribe {}: {}",
                            path.display(),
                            e
                        );
                        return default;
                    }
                    if let Err(e) = save_config_to(path, &default) {
                        tracing::error!(
                            "No se pudo escribir la configuración en {}: {}",
                            path.display(),
                            e
                        );
                    }
                    default
                }
            }
            Err(e) => {
                tracing::error!("Error parseando config: {}. Usando defaults", e);
                let default = AppConfig::default();
                if let Err(e) = backup_config_file(path, CONFIG_BACKUP_KEEP) {
                    tracing::error!(
                        "Backup de config falló, no se sobrescribe {}: {}",
                        path.display(),
                        e
                    );
                    return default;
                }
                if let Err(e) = save_config_to(path, &default) {
                    tracing::error!(
                        "No se pudo escribir la configuración en {}: {}",
                        path.display(),
                        e
                    );
                }
                default
            }
        },
        Err(e) => {
            tracing::warn!("No se pudo leer config: {}. Usando defaults", e);
            AppConfig::default()
        }
    }
}

/// Carga la configuración desde la ruta por defecto.
pub fn load_config() -> AppConfig {
    load_config_from(&default_config_path())
}

/// Guarda la configuración en la ruta indicada.
pub fn save_config_to(path: &Path, config: &AppConfig) -> Result<(), ConfigError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| ConfigError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let content = ron::ser::to_string_pretty(config, ron::ser::PrettyConfig::default())
        .map_err(|source| ConfigError::Serialize { source })?;
    std::fs::write(path, content).map_err(|source| ConfigError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(())
}

/// Guarda la configuración en la ruta por defecto.
pub fn save_config(config: &AppConfig) -> Result<(), ConfigError> {
    save_config_to(&default_config_path(), config)
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
    if let Some(last) = *last_mtime
        && modified <= last
    {
        return None; // No cambió
    }
    *last_mtime = Some(modified);

    // Recargar
    match std::fs::read_to_string(&path) {
        Ok(content) => match ron::from_str(&content) {
            Ok(config) => {
                tracing::info!("Configuración recargada desde disco");
                resolve_resource_profile(&config);
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

pub fn resolve_resource_profile(config: &AppConfig) {
    let active = match config.profile {
        ConfigProfile::LowResource => true,
        // `Default` significa "automático": delega en la sonda de hardware en
        // vez de forzar el flag a `false`. Forzarlo apagaba el modo de bajos
        // recursos en cada recarga de configuración, aunque el equipo lo
        // necesitara, y con él crecían el ring buffer, los márgenes del
        // viewport y los pools de carátulas.
        ConfigProfile::Default => crate::utils::hardware_low_resource(),
        // Una elección explícita de máxima fidelidad sí desactiva el modo.
        ConfigProfile::HighFidelity => false,
    };
    crate::utils::set_effective_low_resource(active);
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod config_tests;
