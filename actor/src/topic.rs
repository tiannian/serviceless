use service_channel::oneshot;
use std::collections::BTreeMap;

/// A typed pub/sub topic.
pub trait Topic: Ord + Clone + Send + 'static {
    type Item: Clone + Send + 'static;
}

/// Bind a topic to a concrete endpoint field on a service.
///
/// This is the key piece that replaces Any/TypeId routing:
/// each topic knows where its endpoint lives on service S.
pub trait RoutedTopic<S>: Topic
where
    S: crate::Service,
{
    /// Returns this topic's [`TopicEndpoint`] on `service`.
    ///
    /// Implementations should consistently point at the same logical field on `S` so
    /// routing matches how the service stores topic state.
    fn endpoint(service: &mut S) -> &mut TopicEndpoint<Self>
    where
        Self: Sized;
}

/// A single-shot broadcast endpoint.
///
/// - each subscribe registers one waiter
/// - each publish wakes all current waiters once
/// - future publishes require future subscribe calls again
pub struct TopicEndpoint<T>
where
    T: Topic,
{
    waiters: BTreeMap<T, Vec<oneshot::Sender<T::Item>>>,
}

impl<T> Default for TopicEndpoint<T>
where
    T: Topic,
{
    /// Empty endpoint with no waiters.
    fn default() -> Self {
        Self {
            waiters: BTreeMap::new(),
        }
    }
}

impl<T> TopicEndpoint<T>
where
    T: Topic,
{
    /// Register one subscriber waiting for the next publication.
    pub fn subscribe(&mut self, topic: T, tx: oneshot::Sender<T::Item>) {
        self.waiters.entry(topic).or_default().push(tx);
    }

    /// Publish once to all current subscribers, then clear them.
    ///
    /// Subscribers that already dropped are silently skipped.
    pub fn publish(&mut self, topic: &T, item: T::Item) {
        let waiters = self.waiters.remove(topic).unwrap_or_default();

        for tx in waiters {
            let _ = tx.send(item.clone());
        }
    }

    /// Number of topic values that currently have at least one waiter.
    pub fn topic_len(&self) -> usize {
        self.waiters.len()
    }

    /// Number of registered one-shot subscribers waiting for the next publish on `topic`.
    pub fn len(&self, topic: &T) -> usize {
        self.waiters.get(topic).map_or(0, Vec::len)
    }

    /// Returns `true` when there are no pending subscribers.
    pub fn is_empty(&self) -> bool {
        self.waiters.is_empty()
    }

    /// Returns `true` when there are no pending subscribers on `topic`.
    pub fn is_topic_empty(&self, topic: &T) -> bool {
        self.waiters.get(topic).is_none_or(Vec::is_empty)
    }

    /// Remove all pending subscribers without publishing.
    ///
    /// Dropped [`oneshot::Sender`]s cause the corresponding receivers to see a closed channel.
    pub fn clear(&mut self) {
        self.waiters.clear();
    }

    /// Remove all pending subscribers for a specific topic value.
    pub fn clear_topic(&mut self, topic: &T) {
        self.waiters.remove(topic);
    }
}
