use std::error::Error;

/// A service is an running like thread
pub trait Service: Send + Sync {
    /// Error of service
    type Error: Error + Send + Sync + 'static;

    /// Start service
    fn start(&mut self) -> Result<(), Self::Error>;

    /// Stop service
    fn stop(&mut self) -> Result<(), Self::Error>;
}
