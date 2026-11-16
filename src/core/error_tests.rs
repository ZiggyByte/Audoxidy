//! Tests for `CoreError` display text and its `From` conversions.
//!
//! Every assertion is a pure value comparison against an error constructed
//! directly, so nothing here opens a database, a socket or a file. The display
//! wording and the conversion targets are the contract a front-end relies on
//! when it reports a failed command.

use super::*;

#[test]
fn test_core_error_display_database() {
    let error = CoreError::Database(rusqlite::Error::ExecuteReturnedResults);
    let message = error.to_string();
    assert!(message.contains("database error"));
}

#[test]
fn test_core_error_display_not_found() {
    let error = CoreError::NotFound("song".into());
    assert_eq!(error.to_string(), "not found: song");
}

#[test]
fn test_core_error_from_sqlite() {
    let sqlite_error = rusqlite::Error::ExecuteReturnedResults;
    let core_error: CoreError = sqlite_error.into();
    assert!(matches!(core_error, CoreError::Database(_)));
}

#[test]
fn test_core_error_from_io() {
    let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
    let core_error: CoreError = io_error.into();
    assert!(matches!(core_error, CoreError::Io(_)));
}
