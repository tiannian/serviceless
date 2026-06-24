use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Unbounded channel closed")]
    UnboundedChannelClosed,
    #[error("Oneshot channel closed")]
    OneshotChannelClosed,
}
