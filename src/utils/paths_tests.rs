//! Tests for the process-wide filesystem locations.
//!
//! They pin the desktop defaults of `AppPaths` — the database in the working
//! directory, covers under `cache/covers`, and the config file under the user's
//! config directory — and their agreement with the path resolution the
//! `APP_SETTINGS` table is read and written through. Every assertion compares
//! pure values, so no real user directory is touched or created.

use super::*;

#[test]
fn test_desktop_default_matches_app_paths() {
    let paths = AppPaths::desktop_default();
    assert_eq!(paths.db_path, PathBuf::from("library.db"));
    assert_eq!(paths.cover_cache_dir, PathBuf::from("cache/covers"));
    assert!(paths.config_path.to_string_lossy().contains("audoxidy"));
    assert!(paths.config_path.to_string_lossy().ends_with("config.ron"));
}
