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

pub mod mpsc;
mod queue;
mod sink;

pub use mpsc::{
    unbounded, Recv, RecvError, SendError, TryRecvError, TrySendError, UnboundedReceiver,
    UnboundedSender,
};
