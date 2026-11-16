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
    /// A shared lock could not be acquired because it was poisoned.
    LockUnavailable,
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
            Self::LockUnavailable => write!(f, "database lock unavailable"),
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
#[path = "error_tests.rs"]
mod tests;
