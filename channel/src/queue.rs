//! A mostly lock-free multi-producer, single consumer queue for sending
//! messages between asynchronous tasks.
//!
//! The queue implementation is essentially the same one used for mpsc channels
//! in the standard library.
//!
//! Note that the current implementation of this queue has a caveat of the `pop`
//! method, and see the method for more information about it. Due to this
//! caveat, this queue may not be appropriate for all use-cases.
//!
//! ## Source
//!
//! This implementation is derived from the `futures-channel` crate.
//! Original code: https://github.com/rust-lang/futures-rs/tree/master/futures-channel/src/mpsc
//!
//! The underlying lock-free algorithm is based on:
//! http://www.1024cores.net/home/lock-free-algorithms/queues/non-intrusive-mpsc-node-based-queue

pub(crate) use self::PopResult::*;

use alloc::boxed::Box;
use core::cell::UnsafeCell;
use core::ptr;
use core::sync::atomic::{AtomicPtr, Ordering};

/// A result of the `pop` function.
pub(crate) enum PopResult<T> {
    /// Some data has been popped
    Data(T),
    /// The queue is empty
    Empty,
    /// The queue is in an inconsistent state. Popping data should succeed, but
    /// some pushers have yet to make enough progress in order allow a pop to
    /// succeed. It is recommended that a pop() occur "in the near future" in
    /// order to see if the sender has made progress or not
    Inconsistent,
}

struct Node<T> {
    next: AtomicPtr<Self>,
    value: Option<T>,
}

/// The multi-producer single-consumer structure. This is not cloneable, but it
/// may be safely shared so long as it is guaranteed that there is only one
/// popper at a time (many pushers are allowed).
pub(crate) struct Queue<T> {
    head: AtomicPtr<Node<T>>,
    tail: UnsafeCell<*mut Node<T>>,
}

unsafe impl<T: Send> Send for Queue<T> {}
unsafe impl<T: Send> Sync for Queue<T> {}

impl<T> Node<T> {
    unsafe fn new(v: Option<T>) -> *mut Self {
        Box::into_raw(Box::new(Self {
            next: AtomicPtr::new(ptr::null_mut()),
            value: v,
        }))
    }
}

impl<T> Queue<T> {
    /// Creates a new queue that is safe to share among multiple producers and
    /// one consumer.
    pub(crate) fn new() -> Self {
        let stub = unsafe { Node::new(None) };
        Self {
            head: AtomicPtr::new(stub),
            tail: UnsafeCell::new(stub),
        }
    }

    /// Pushes a new value onto this queue.
    pub(crate) fn push(&self, t: T) {
        unsafe {
            let n = Node::new(Some(t));
            let prev = self.head.swap(n, Ordering::AcqRel);
            (*prev).next.store(n, Ordering::Release);
        }
    }

    /// Pops some data from this queue.
    ///
    /// Note that the current implementation means that this function cannot
    /// return `Option<T>`. It is possible for this queue to be in an
    /// inconsistent state where many pushes have succeeded and completely
    /// finished, but pops cannot return `Some(t)`. This inconsistent state
    /// happens when a pusher is preempted at an inopportune moment.
    ///
    /// This inconsistent state means that this queue does indeed have data, but
    /// it does not currently have access to it at this time.
    ///
    /// This function is unsafe because only one thread can call it at a time.
    pub(crate) unsafe fn pop(&self) -> PopResult<T> {
        unsafe {
            let tail = *self.tail.get();
            let next = (*tail).next.load(Ordering::Acquire);

            if !next.is_null() {
                *self.tail.get() = next;
                assert!((*tail).value.is_none());
                assert!((*next).value.is_some());
                let ret = (*next).value.take().unwrap();
                drop(Box::from_raw(tail));
                return Data(ret);
            }

            if self.head.load(Ordering::Acquire) == tail {
                Empty
            } else {
                Inconsistent
            }
        }
    }

    /// Pop an element similarly to `pop` function, but spin-wait on inconsistent
    /// queue state instead of returning `Inconsistent`.
    ///
    /// This function is unsafe because only one thread can call it at a time.
    pub(crate) unsafe fn pop_spin(&self) -> Option<T> {
        loop {
            match unsafe { self.pop() } {
                Empty => return None,
                Data(t) => return Some(t),
                // Inconsistent means that there will be a message to pop
                // in a short time. This branch can only be reached if
                // values are being produced from another thread, so there
                // are a few ways that we can deal with this:
                //
                // 1) Spin
                // 2) core::hint::spin_loop()
                // 3) task::current().unwrap() & return Pending
                //
                // For now, core::hint::spin_loop() is used, but it would
                // probably be better to spin a few times then yield.
                Inconsistent => {
                    core::hint::spin_loop();
                }
            }
        }
    }
}

impl<T> Drop for Queue<T> {
    fn drop(&mut self) {
        unsafe {
            let mut cur = *self.tail.get();
            while !cur.is_null() {
                let next = (*cur).next.load(Ordering::Relaxed);
                drop(Box::from_raw(cur));
                cur = next;
            }
        }
    }
}
