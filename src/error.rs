use thiserror::Error;

/// Error
#[derive(Debug, Error)]
pub enum Error {
    /// Service already stoped
    #[error("Service already stoped")]
    ServiceStoped,

    /// This query is send, can't read result
    #[error("Service is paused")]
    ServicePaused,
}

/// Result
pub type Result<T> = std::result::Result<T, Error>;
