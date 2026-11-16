//! Tests for `CorePaths`: the desktop defaults and the clone.
//!
//! Both tests compare plain values — the database in the working directory,
//! covers under `cache/covers` and the config file under the user's config
//! directory — so no directory or file is created, opened or read.

use super::*;

#[test]
fn test_core_paths_desktop_default() {
    let paths = CorePaths::desktop_default();
    assert_eq!(paths.db_path, PathBuf::from("library.db"));
    assert_eq!(paths.cover_cache_dir, PathBuf::from("cache/covers"));
    assert!(paths.config_path.to_string_lossy().contains("audoxidy"));
    assert!(paths.config_path.to_string_lossy().ends_with("config.ron"));
}

#[test]
fn test_core_paths_clone() {
    let paths = CorePaths::desktop_default();
    let cloned = paths.clone();
    assert_eq!(paths.db_path, cloned.db_path);
    assert_eq!(paths.cover_cache_dir, cloned.cover_cache_dir);
    assert_eq!(paths.config_path, cloned.config_path);
}
