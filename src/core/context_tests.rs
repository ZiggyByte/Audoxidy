//! Tests for `Core` construction and the injected path handles.
//!
//! The test assembles a `Core` from an in-memory database, a scanner and the
//! audio manager, then reads the locations back through the accessor. It pins
//! the constructor wiring only: no database file, cover directory or
//! configuration file is created, opened or read.

use super::*;
use crate::core::types::CorePaths;

#[test]
fn test_core_constructs_with_default_paths() {
    let paths = CorePaths::desktop_default();
    let db = Arc::new(Mutex::new(Database::new_memory().expect("in-memory db")));
    let scanner = Arc::new(Scanner::new(db.clone()));
    let audio_manager = Arc::new(AudioManager::new().expect("audio manager init"));

    let core = Core::new(audio_manager, db, scanner, paths.clone());
    assert_eq!(core.paths().db_path, paths.db_path);
    assert_eq!(core.paths().cover_cache_dir, paths.cover_cache_dir);
    assert_eq!(core.paths().config_path, paths.config_path);
}
