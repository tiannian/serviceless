/// Error
#[derive(Debug)]
pub enum Error {
    /// Service already stoped
    ServiceStoped,

    /// This query is send, can't read result
    ServicePaused,
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Error::ServiceStoped => write!(f, "Service already stoped"),
            Error::ServicePaused => write!(f, "Service is paused"),
        }
    }
}

impl core::error::Error for Error {}

/// Result
pub type Result<T> = core::result::Result<T, Error>;
