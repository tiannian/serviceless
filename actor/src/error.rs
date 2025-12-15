#[cfg(feature = "std")]
use thiserror::Error;

#[cfg(feature = "std")]
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

#[cfg(not(feature = "std"))]
/// Error
#[derive(Debug)]
pub enum Error {
    /// Service already stoped
    ServiceStoped,

    /// This query is send, can't read result
    ServicePaused,
}

#[cfg(not(feature = "std"))]
impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Error::ServiceStoped => write!(f, "Service already stoped"),
            Error::ServicePaused => write!(f, "Service is paused"),
        }
    }
}

#[cfg(not(feature = "std"))]
impl core::error::Error for Error {}

/// Result
#[cfg(feature = "std")]
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(not(feature = "std"))]
pub type Result<T> = core::result::Result<T, Error>;
