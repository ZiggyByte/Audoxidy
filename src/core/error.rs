//! Typed error type for Core operations.
//!
//! Every Core method returns `Result<T, CoreError>` instead of propagating
//! string errors. The enum mirrors the error surfaces that the underlying
//! services expose and provides `Display` + `Error` implementations plus
//! `From` conversions for the standard errors it wraps.

use std::fmt;

/// Errors that Core operations can produce.
#[derive(Debug)]
pub enum CoreError {
    /// A database operation failed.
    Database(rusqlite::Error),
    /// An audio subsystem operation failed.
    Audio(String),
    /// An I/O operation failed.
    Io(std::io::Error),
    /// A requested resource was not found.
    NotFound(String),
    /// A resource already exists.
    AlreadyExists(String),
    /// An input was invalid.
    InvalidInput(String),
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(e) => write!(f, "database error: {e}"),
            Self::Audio(msg) => write!(f, "audio error: {msg}"),
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::NotFound(msg) => write!(f, "not found: {msg}"),
            Self::AlreadyExists(msg) => write!(f, "already exists: {msg}"),
            Self::InvalidInput(msg) => write!(f, "invalid input: {msg}"),
        }
    }
}

impl std::error::Error for CoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Database(e) => Some(e),
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<rusqlite::Error> for CoreError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Database(error)
    }
}

impl From<std::io::Error> for CoreError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

#[cfg(test)]
mod tests {
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
}
