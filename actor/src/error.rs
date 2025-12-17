use thiserror::Error;

/// Error
#[derive(Debug, Error)]
pub enum Error {
    /// Service already stoped
    #[error("Service already stoped")]
    ServiceStoped,
}

/// Result
pub type Result<T> = std::result::Result<T, Error>;
