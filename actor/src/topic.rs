use service_channel::oneshot;

/// A typed pub/sub topic.
pub trait Topic: Send + 'static {
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
    waiters: Vec<oneshot::Sender<T::Item>>,
}

impl<T> Default for TopicEndpoint<T>
where
    T: Topic,
{
    /// Empty endpoint with no waiters.
    fn default() -> Self {
        Self {
            waiters: Vec::new(),
        }
    }
}

impl<T> TopicEndpoint<T>
where
    T: Topic,
{
    /// Register one subscriber waiting for the next publication.
    pub fn subscribe(&mut self, tx: oneshot::Sender<T::Item>) {
        self.waiters.push(tx);
    }

    /// Publish once to all current subscribers, then clear them.
    ///
    /// Subscribers that already dropped are silently skipped.
    pub fn publish(&mut self, item: T::Item) {
        let waiters = std::mem::take(&mut self.waiters);

        for tx in waiters {
            let _ = tx.send(item.clone());
        }
    }

    /// Number of registered one-shot subscribers waiting for the next publish.
    pub fn len(&self) -> usize {
        self.waiters.len()
    }

    /// Returns `true` when there are no pending subscribers.
    pub fn is_empty(&self) -> bool {
        self.waiters.is_empty()
    }

    /// Remove all pending subscribers without publishing.
    ///
    /// Dropped [`oneshot::Sender`]s cause the corresponding receivers to see a closed channel.
    pub fn clear(&mut self) {
        self.waiters.clear();
    }
}
