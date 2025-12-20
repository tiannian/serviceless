//! A multi-producer, single-consumer channel for sending values across
//! asynchronous tasks.
//!
//! This crate provides an unbounded channel implementation based on futures-rs.
//!
//! ## Source
//!
//! This implementation is derived from the `futures-channel` crate.
//! Original code: https://github.com/rust-lang/futures-rs/tree/master/futures-channel

#![no_std]

extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

mod lock;
pub mod mpsc;
pub mod oneshot;
mod queue;
mod sink;

pub use mpsc::{
    unbounded, Recv, RecvError, SendError, TryRecvError, TrySendError, UnboundedReceiver,
    UnboundedSender,
};
pub use oneshot::{channel, Canceled, Receiver, Sender};
