use std::collections::BTreeMap;

use crate::{
    runtime::{OneshotSender, UnboundedSender},
    Topic,
};

/// A single-shot broadcast endpoint.
///
/// - each subscribe registers one waiter
/// - each publish wakes all current waiters once
/// - future publishes require future subscribe calls again
pub struct RuntimedTopicEndpoint<T, O, R>
where
    T: Topic,
    O: OneshotSender<T::Item>,
    R: UnboundedSender<T::Item>,
{
    once_waiters: BTreeMap<T, Vec<O>>,
    all_waiters: BTreeMap<T, Vec<R>>,
}

impl<T, O, R> Default for RuntimedTopicEndpoint<T, O, R>
where
    T: Topic,
    O: OneshotSender<T::Item>,
    R: UnboundedSender<T::Item>,
{
    /// Empty endpoint with no waiters.
    fn default() -> Self {
        Self {
            once_waiters: BTreeMap::new(),
            all_waiters: BTreeMap::new(),
        }
    }
}

impl<T, O, R> RuntimedTopicEndpoint<T, O, R>
where
    T: Topic,
    O: OneshotSender<T::Item>,
    R: UnboundedSender<T::Item>,
{
    /// Register one subscriber waiting for the next publication.
    pub fn subscribe(&mut self, topic: T, tx: O) {
        self.once_waiters.entry(topic).or_default().push(tx);
    }

    /// Register a subscriber waiting for all future publications.
    pub fn subscribe_all(&mut self, topic: T, tx: R) {
        self.all_waiters.entry(topic).or_default().push(tx);
    }

    /// Publish once to all current subscribers, then clear them.
    ///
    /// Subscribers that already dropped are silently skipped.
    pub fn publish(&mut self, topic: &T, item: T::Item) {
        let waiters = self.once_waiters.remove(topic).unwrap_or_default();

        for tx in waiters {
            let _ = tx.send(item.clone());
        }

        let waiters = self.all_waiters.get_mut(topic);

        if let Some(waiters) = waiters {
            for tx in waiters.iter() {
                let _ = tx.send(item.clone());
            }

            waiters.retain(|tx| !tx.is_closed());
        }
    }
}
