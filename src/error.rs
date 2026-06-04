//! Error and Result types used across the crate.

use thiserror::Error;

/// All the ways a cleat operation can fail.
#[derive(Debug, Error)]
pub enum Error {
    #[error("class not found: {0}")]
    ClassNotFound(String),

    #[error("assembly not found: {0}")]
    AssemblyNotFound(String),

    #[error("field not found: {0}")]
    FieldNotFound(String),

    #[error("method not found: {0}")]
    MethodNotFound(String),

    #[error("cleat not initialized: call set_app_data() first")]
    NotInitialized,

    #[error("bridge error: {0}")]
    Bridge(String),

    #[error("hook error: {0}")]
    Hook(String),
}

/// The standard result alias used everywhere in cleat.
pub type Result<T> = std::result::Result<T, Error>;
