//! A multi-producer, single-consumer queue for sending values across
//! asynchronous tasks.
//!
//! This module provides an unbounded channel implementation based on futures-rs.
//!
//! ## Source
//!
//! This implementation is derived from the `futures-channel` crate.
//! Original code: https://github.com/rust-lang/futures-rs/tree/master/futures-channel/src/mpsc

use alloc::sync::Arc;
use core::fmt;
use core::future::Future;
use core::pin::Pin;
use core::sync::atomic::AtomicUsize;
use core::sync::atomic::Ordering::SeqCst;
use core::task::{Context, Poll};
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::__internal::AtomicWaker;
use futures_core::FusedFuture;

use crate::queue::Queue;

struct UnboundedSenderInner<T> {
    // Channel state shared between the sender and receiver.
    inner: Arc<UnboundedInner<T>>,
}

// We never project Pin<&mut SenderInner> to `Pin<&mut T>`
impl<T> Unpin for UnboundedSenderInner<T> {}

/// The transmission end of an unbounded mpsc channel.
///
/// This value is created by the [`unbounded`] function.
pub struct UnboundedSender<T>(Option<UnboundedSenderInner<T>>);

/// The receiving end of an unbounded mpsc channel.
///
/// This value is created by the [`unbounded`] function.
pub struct UnboundedReceiver<T> {
    inner: Option<Arc<UnboundedInner<T>>>,
}

// `Pin<&mut UnboundedReceiver<T>>` is never projected to `Pin<&mut T>`
impl<T> Unpin for UnboundedReceiver<T> {}

/// The error type for [`UnboundedSender`s](UnboundedSender) used as `Sink`s.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SendError {
    kind: SendErrorKind,
}

/// The error type returned from [`try_send`](UnboundedSender::unbounded_send).
#[derive(Clone, PartialEq, Eq)]
pub struct TrySendError<T> {
    err: SendError,
    val: T,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum SendErrorKind {
    Disconnected,
}

/// Error returned by [`UnboundedReceiver::try_recv`].
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum TryRecvError {
    /// The channel is empty but not closed.
    Empty,

    /// The channel is empty and closed.
    Closed,
}

/// Error returned by the future returned by [`UnboundedReceiver::recv()`].
/// Received when the channel is empty and closed.
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub struct RecvError;

/// Future returned by [`UnboundedReceiver::recv()`].
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct Recv<'a, St: ?Sized> {
    stream: &'a mut St,
}

impl fmt::Display for SendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "send failed because receiver is gone")
    }
}

impl core::error::Error for SendError {}

impl fmt::Display for RecvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "receive failed because channel is empty and closed")
    }
}

impl core::error::Error for RecvError {}

impl SendError {
    /// Returns `true` if this error is a result of the receiver being dropped.
    pub fn is_disconnected(&self) -> bool {
        matches!(self.kind, SendErrorKind::Disconnected)
    }
}

impl TryRecvError {
    /// Returns `true` if the channel is empty but not closed.
    pub fn is_empty(&self) -> bool {
        matches!(self, TryRecvError::Empty)
    }

    /// Returns `true` if the channel is empty and closed.
    pub fn is_closed(&self) -> bool {
        matches!(self, TryRecvError::Closed)
    }
}

impl<T> fmt::Debug for TrySendError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TrySendError")
            .field("kind", &self.err.kind)
            .finish()
    }
}

impl<T> fmt::Display for TrySendError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "send failed because receiver is gone")
    }
}

impl<T: core::any::Any> core::error::Error for TrySendError<T> {}

impl<T> TrySendError<T> {
    /// Returns `true` if this error is a result of the receiver being dropped.
    pub fn is_disconnected(&self) -> bool {
        self.err.is_disconnected()
    }

    /// Returns the message that was attempted to be sent but failed.
    pub fn into_inner(self) -> T {
        self.val
    }

    /// Drops the message and converts into a `SendError`.
    pub fn into_send_error(self) -> SendError {
        self.err
    }
}

impl fmt::Display for TryRecvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TryRecvError::Empty => write!(f, "receive failed because channel is empty"),
            TryRecvError::Closed => write!(f, "receive failed because channel is closed"),
        }
    }
}

impl core::error::Error for TryRecvError {}

struct UnboundedInner<T> {
    // Internal channel state. Consists of the number of messages stored in the
    // channel as well as a flag signalling that the channel is closed.
    state: AtomicUsize,

    // Atomic, FIFO queue used to send messages to the receiver
    message_queue: Queue<T>,

    // Number of senders in existence
    num_senders: AtomicUsize,

    // Handle to the receiver's task.
    recv_task: AtomicWaker,
}

// Struct representation of `Inner::state`.
#[derive(Clone, Copy)]
struct State {
    // `true` when the channel is open
    is_open: bool,

    // Number of messages in the channel
    num_messages: usize,
}

// The `is_open` flag is stored in the left-most bit of `Inner::state`
const OPEN_MASK: usize = usize::MAX - (usize::MAX >> 1);

// When a new channel is created, it is created in the open state with no
// pending messages.
const INIT_STATE: usize = OPEN_MASK;

// The maximum number of messages that a channel can track is `usize::MAX >> 1`
const MAX_CAPACITY: usize = !(OPEN_MASK);

// The maximum requested buffer size must be less than the maximum capacity of
// a channel. This is because each sender gets a guaranteed slot.
const MAX_BUFFER: usize = MAX_CAPACITY >> 1;

/// Creates an unbounded mpsc channel for communicating between asynchronous
/// tasks.
///
/// A `send` on this channel will always succeed as long as the receive half has
/// not been closed. If the receiver falls behind, messages will be arbitrarily
/// buffered.
///
/// **Note** that the amount of available system memory is an implicit bound to
/// the channel. Using an `unbounded` channel has the ability of causing the
/// process to run out of memory. In this case, the process will be aborted.
pub fn unbounded<T>() -> (UnboundedSender<T>, UnboundedReceiver<T>) {
    let inner = Arc::new(UnboundedInner {
        state: AtomicUsize::new(INIT_STATE),
        message_queue: Queue::new(),
        num_senders: AtomicUsize::new(1),
        recv_task: AtomicWaker::new(),
    });

    let tx = UnboundedSenderInner {
        inner: inner.clone(),
    };

    let rx = UnboundedReceiver { inner: Some(inner) };

    (UnboundedSender(Some(tx)), rx)
}

/*
 *
 * ===== impl UnboundedSenderInner =====
 *
 */

impl<T> UnboundedSenderInner<T> {
    fn poll_ready_nb(&self) -> Poll<Result<(), SendError>> {
        let state = decode_state(self.inner.state.load(SeqCst));
        if state.is_open {
            Poll::Ready(Ok(()))
        } else {
            Poll::Ready(Err(SendError {
                kind: SendErrorKind::Disconnected,
            }))
        }
    }

    // Push message to the queue and signal to the receiver
    fn queue_push_and_signal(&self, msg: T) {
        // Push the message onto the message queue
        self.inner.message_queue.push(msg);

        // Signal to the receiver that a message has been enqueued. If the
        // receiver is parked, this will unpark the task.
        self.inner.recv_task.wake();
    }

    // Increment the number of queued messages. Returns the resulting number.
    fn inc_num_messages(&self) -> Option<usize> {
        let mut curr = self.inner.state.load(SeqCst);

        loop {
            let mut state = decode_state(curr);

            // The receiver end closed the channel.
            if !state.is_open {
                return None;
            }

            // This probably is never hit? Odds are the process will run out of
            // memory first. It may be worth to return something else in this
            // case?
            assert!(
                state.num_messages < MAX_CAPACITY,
                "buffer space \
                    exhausted; sending this messages would overflow the state"
            );

            state.num_messages += 1;

            let next = encode_state(&state);
            match self
                .inner
                .state
                .compare_exchange(curr, next, SeqCst, SeqCst)
            {
                Ok(_) => return Some(state.num_messages),
                Err(actual) => curr = actual,
            }
        }
    }

    /// Returns whether the senders send to the same receiver.
    fn same_receiver(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }

    /// Returns whether the sender send to this receiver.
    fn is_connected_to(&self, inner: &Arc<UnboundedInner<T>>) -> bool {
        Arc::ptr_eq(&self.inner, inner)
    }

    /// Returns pointer to the Arc containing sender
    ///
    /// The returned pointer is not referenced and should be only used for hashing!
    fn ptr(&self) -> *const UnboundedInner<T> {
        &*self.inner
    }

    /// Returns whether this channel is closed without needing a context.
    fn is_closed(&self) -> bool {
        !decode_state(self.inner.state.load(SeqCst)).is_open
    }

    /// Closes this channel from the sender side, preventing any new messages.
    fn close_channel(&self) {
        // There's no need to park this sender, its dropping,
        // and we don't want to check for capacity, so skip
        // that stuff from `do_send`.

        self.inner.set_closed();
        self.inner.recv_task.wake();
    }
}

/*
 *
 * ===== impl UnboundedSender =====
 *
 */

impl<T> UnboundedSender<T> {
    /// Check if the channel is ready to receive a message.
    pub fn poll_ready(&self, _: &mut Context<'_>) -> Poll<Result<(), SendError>> {
        let inner = self.0.as_ref().ok_or(SendError {
            kind: SendErrorKind::Disconnected,
        })?;
        inner.poll_ready_nb()
    }

    /// Returns whether this channel is closed without needing a context.
    pub fn is_closed(&self) -> bool {
        self.0
            .as_ref()
            .map(UnboundedSenderInner::is_closed)
            .unwrap_or(true)
    }

    /// Closes this channel from the sender side, preventing any new messages.
    pub fn close_channel(&self) {
        if let Some(inner) = &self.0 {
            inner.close_channel();
        }
    }

    /// Disconnects this sender from the channel, closing it if there are no more senders left.
    pub fn disconnect(&mut self) {
        self.0 = None;
    }

    // Do the send without parking current task.
    fn do_send_nb(&self, msg: T) -> Result<(), TrySendError<T>> {
        if let Some(inner) = &self.0 {
            if inner.inc_num_messages().is_some() {
                inner.queue_push_and_signal(msg);
                return Ok(());
            }
        }

        Err(TrySendError {
            err: SendError {
                kind: SendErrorKind::Disconnected,
            },
            val: msg,
        })
    }

    /// Send a message on the channel.
    ///
    /// This method should only be called after `poll_ready` has been used to
    /// verify that the channel is ready to receive a message.
    pub fn start_send(&mut self, msg: T) -> Result<(), SendError> {
        self.do_send_nb(msg).map_err(|e| e.err)
    }

    /// Sends a message along this channel.
    ///
    /// This is an unbounded sender, so this function differs from `Sink::send`
    /// by ensuring the return type reflects that the channel is always ready to
    /// receive messages.
    pub fn unbounded_send(&self, msg: T) -> Result<(), TrySendError<T>> {
        self.do_send_nb(msg)
    }

    /// Returns whether the senders send to the same receiver.
    pub fn same_receiver(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (Some(inner), Some(other)) => inner.same_receiver(other),
            _ => false,
        }
    }

    /// Returns whether the sender send to this receiver.
    pub fn is_connected_to(&self, receiver: &UnboundedReceiver<T>) -> bool {
        match (&self.0, &receiver.inner) {
            (Some(inner), Some(receiver)) => inner.is_connected_to(receiver),
            _ => false,
        }
    }

    /// Hashes the receiver into the provided hasher
    pub fn hash_receiver<H>(&self, hasher: &mut H)
    where
        H: core::hash::Hasher,
    {
        use core::hash::Hash;

        let ptr = self.0.as_ref().map(|inner| inner.ptr());
        ptr.hash(hasher);
    }

    /// Return the number of messages in the queue or 0 if channel is disconnected.
    pub fn len(&self) -> usize {
        if let Some(sender) = &self.0 {
            decode_state(sender.inner.state.load(SeqCst)).num_messages
        } else {
            0
        }
    }

    /// Return false is channel has no queued messages, true otherwise.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<T> Clone for UnboundedSender<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> Clone for UnboundedSenderInner<T> {
    fn clone(&self) -> Self {
        // Since this atomic op isn't actually guarding any memory and we don't
        // care about any orderings besides the ordering on the single atomic
        // variable, a relaxed ordering is acceptable.
        let mut curr = self.inner.num_senders.load(SeqCst);

        loop {
            // If the maximum number of senders has been reached, then fail
            if curr == MAX_BUFFER {
                panic!("cannot clone `Sender` -- too many outstanding senders");
            }

            debug_assert!(curr < MAX_BUFFER);

            let next = curr + 1;
            match self
                .inner
                .num_senders
                .compare_exchange(curr, next, SeqCst, SeqCst)
            {
                Ok(_) => {
                    // The ABA problem doesn't matter here. We only care that the
                    // number of senders never exceeds the maximum.
                    return Self {
                        inner: self.inner.clone(),
                    };
                }
                Err(actual) => curr = actual,
            }
        }
    }
}

impl<T> Drop for UnboundedSenderInner<T> {
    fn drop(&mut self) {
        // Ordering between variables don't matter here
        let prev = self.inner.num_senders.fetch_sub(1, SeqCst);

        if prev == 1 {
            self.close_channel();
        }
    }
}

impl<T> fmt::Debug for UnboundedSender<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UnboundedSender")
            .field("closed", &self.is_closed())
            .finish()
    }
}

/*
 *
 * ===== impl UnboundedReceiver =====
 *
 */

impl<T> UnboundedReceiver<T> {
    /// Waits for a message from the channel.
    /// If the channel is empty and closed, returns [`RecvError`].
    pub fn recv(&mut self) -> Recv<'_, Self> {
        Recv::new(self)
    }

    /// Closes the receiving half of a channel, without dropping it.
    ///
    /// This prevents any further messages from being sent on the channel while
    /// still enabling the receiver to drain messages that are buffered.
    pub fn close(&mut self) {
        if let Some(inner) = &mut self.inner {
            inner.set_closed();
        }
    }

    /// Tries to receive the next message without notifying a context if empty.
    ///
    /// It is not recommended to call this function from inside of a future,
    /// only when you've otherwise arranged to be notified when the channel is
    /// no longer empty.
    ///
    /// This function returns:
    /// * `Ok(Some(t))` when message is fetched
    /// * `Ok(None)` when channel is closed and no messages left in the queue
    /// * `Err(e)` when there are no messages available, but channel is not yet closed
    #[deprecated(note = "please use `try_recv` instead")]
    pub fn try_next(&mut self) -> Result<Option<T>, TryRecvError> {
        match self.next_message() {
            Poll::Ready(msg) => Ok(msg),
            Poll::Pending => Err(TryRecvError::Empty),
        }
    }

    /// Tries to receive a message from the channel without blocking.
    /// If the channel is empty, or empty and closed, this method returns an error.
    pub fn try_recv(&mut self) -> Result<T, TryRecvError> {
        match self.next_message() {
            Poll::Ready(Some(msg)) => Ok(msg),
            Poll::Ready(None) => Err(TryRecvError::Closed),
            Poll::Pending => Err(TryRecvError::Empty),
        }
    }

    fn next_message(&mut self) -> Poll<Option<T>> {
        let inner = match self.inner.as_mut() {
            None => return Poll::Ready(None),
            Some(inner) => inner,
        };
        // Pop off a message
        match unsafe { inner.message_queue.pop_spin() } {
            Some(msg) => {
                // Decrement number of messages
                self.dec_num_messages();

                Poll::Ready(Some(msg))
            }
            None => {
                let state = decode_state(inner.state.load(SeqCst));
                if state.is_closed() {
                    // If closed flag is set AND there are no pending messages
                    // it means end of stream
                    self.inner = None;
                    Poll::Ready(None)
                } else {
                    // If queue is open, we need to return Pending
                    // to be woken up when new messages arrive.
                    // If queue is closed but num_messages is non-zero,
                    // it means that senders updated the state,
                    // but didn't put message to queue yet,
                    // so we need to park until sender unparks the task
                    // after queueing the message.
                    Poll::Pending
                }
            }
        }
    }

    fn dec_num_messages(&self) {
        if let Some(inner) = &self.inner {
            // OPEN_MASK is highest bit, so it's unaffected by subtraction
            // unless there's underflow, and we know there's no underflow
            // because number of messages at this point is always > 0.
            inner.state.fetch_sub(1, SeqCst);
        }
    }
}

impl<T> FusedStream for UnboundedReceiver<T> {
    fn is_terminated(&self) -> bool {
        self.inner.is_none()
    }
}

impl<T> Stream for UnboundedReceiver<T> {
    type Item = T;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<T>> {
        // Try to read a message off of the message queue.
        match self.next_message() {
            Poll::Ready(msg) => {
                if msg.is_none() {
                    self.inner = None;
                }
                Poll::Ready(msg)
            }
            Poll::Pending => {
                // There are no messages to read, in this case, park.
                self.inner.as_ref().unwrap().recv_task.register(cx.waker());
                // Check queue again after parking to prevent race condition:
                // a message could be added to the queue after previous `next_message`
                // before `register` call.
                self.next_message()
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        if let Some(inner) = &self.inner {
            decode_state(inner.state.load(SeqCst)).size_hint()
        } else {
            (0, Some(0))
        }
    }
}

impl<T> Drop for UnboundedReceiver<T> {
    fn drop(&mut self) {
        // Drain the channel of all pending messages
        self.close();
        if self.inner.is_some() {
            loop {
                match self.next_message() {
                    Poll::Ready(Some(_)) => {}
                    Poll::Ready(None) => break,
                    Poll::Pending => {
                        let state = decode_state(self.inner.as_ref().unwrap().state.load(SeqCst));

                        // If the channel is closed, then there is no need to park.
                        if state.is_closed() {
                            break;
                        }

                        // TODO: Spinning isn't ideal, it might be worth
                        // investigating using a condvar or some other strategy
                        // here. That said, if this case is hit, then another thread
                        // is about to push the value into the queue and this isn't
                        // the only spinlock in the impl right now.
                        core::hint::spin_loop();
                    }
                }
            }
        }
    }
}

impl<T> fmt::Debug for UnboundedReceiver<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let closed = if let Some(ref inner) = self.inner {
            decode_state(inner.state.load(SeqCst)).is_closed()
        } else {
            false
        };

        f.debug_struct("UnboundedReceiver")
            .field("closed", &closed)
            .finish()
    }
}

/*
 *
 * ===== impl UnboundedInner =====
 *
 */

impl<T> UnboundedInner<T> {
    // Clear `open` flag in the state, keep `num_messages` intact.
    fn set_closed(&self) {
        let curr = self.state.load(SeqCst);
        if !decode_state(curr).is_open {
            return;
        }

        self.state.fetch_and(!OPEN_MASK, SeqCst);
    }
}

unsafe impl<T: Send> Send for UnboundedInner<T> {}
unsafe impl<T: Send> Sync for UnboundedInner<T> {}

impl State {
    fn is_closed(&self) -> bool {
        !self.is_open && self.num_messages == 0
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.is_open {
            (self.num_messages, None)
        } else {
            (self.num_messages, Some(self.num_messages))
        }
    }
}

/*
 *
 * ===== Helpers =====
 *
 */

fn decode_state(num: usize) -> State {
    State {
        is_open: num & OPEN_MASK == OPEN_MASK,
        num_messages: num & MAX_CAPACITY,
    }
}

fn encode_state(state: &State) -> usize {
    let mut num = state.num_messages;

    if state.is_open {
        num |= OPEN_MASK;
    }

    num
}

/*
 *
 * ===== impl Recv =====
 *
 */

impl<St: ?Sized + Unpin> Unpin for Recv<'_, St> {}
impl<'a, St: ?Sized + Stream + Unpin> Recv<'a, St> {
    fn new(stream: &'a mut St) -> Self {
        Self { stream }
    }
}

impl<St: ?Sized + FusedStream + Unpin> FusedFuture for Recv<'_, St> {
    fn is_terminated(&self) -> bool {
        self.stream.is_terminated()
    }
}

impl<St: ?Sized + Stream + Unpin> Future for Recv<'_, St> {
    type Output = Result<St::Item, RecvError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match Pin::new(&mut self.stream).poll_next(cx) {
            Poll::Ready(Some(msg)) => Poll::Ready(Ok(msg)),
            Poll::Ready(None) => Poll::Ready(Err(RecvError)),
            Poll::Pending => Poll::Pending,
        }
    }
}
