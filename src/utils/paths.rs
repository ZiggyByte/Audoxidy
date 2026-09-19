//! Process-wide filesystem locations.
//!
//! The crate reads the database, configuration and cover-cache locations from
//! this registry. The application initializes it once at startup from the
//! injected [`crate::core::CorePaths`]; other entry points (tests, tools) fall
//! back to the desktop defaults. This module is part of the leaf `utils` layer,
//! so it imports neither `core` nor any front-end crate.

use std::path::PathBuf;
use std::sync::OnceLock;

/// Filesystem locations shared across the crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPaths {
    /// Path to the library SQLite database.
    pub db_path: PathBuf,
    /// Directory where cover art is cached.
    pub cover_cache_dir: PathBuf,
    /// Path to the application configuration file.
    pub config_path: PathBuf,
}

impl AppPaths {
    /// Returns the desktop default locations.
    ///
    /// The database lives at `library.db` in the working directory, covers are
    /// cached under `cache/covers/`, and the config file is stored in the
    /// user's config directory.
    pub fn desktop_default() -> Self {
        Self {
            db_path: PathBuf::from("library.db"),
            cover_cache_dir: PathBuf::from("cache/covers"),
            config_path: default_config_file_path(),
        }
    }
}

/// Computes the platform default config file path.
///
/// Kept separate from the global registry so [`AppPaths::desktop_default`] can
/// build the fallback without recursing back into the registry.
fn default_config_file_path() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".config/audoxidy/config.ron")
}

/// Process-wide registry of filesystem locations.
static PATHS: OnceLock<AppPaths> = OnceLock::new();

/// Publishes the process-wide paths.
///
/// The first call wins; later calls are ignored, so a second initializer can
/// never redirect an already-running process.
pub fn init(paths: AppPaths) {
    let _ = PATHS.set(paths);
}

/// Returns the process-wide paths.
///
/// When nothing was published, the desktop defaults are installed on first
/// access so tests and alternative entry points keep working.
pub fn get() -> &'static AppPaths {
    PATHS.get_or_init(AppPaths::desktop_default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_desktop_default_matches_app_paths() {
        let paths = AppPaths::desktop_default();
        assert_eq!(paths.db_path, PathBuf::from("library.db"));
        assert_eq!(paths.cover_cache_dir, PathBuf::from("cache/covers"));
        assert!(paths.config_path.to_string_lossy().contains("audoxidy"));
        assert!(paths.config_path.to_string_lossy().ends_with("config.ron"));
    }
}
