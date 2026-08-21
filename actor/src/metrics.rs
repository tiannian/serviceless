use prometheus_client::{
    metrics::{
        counter::Counter,
        gauge::Gauge,
        histogram::{exponential_buckets, Histogram},
    },
    registry::Registry,
};

pub(crate) struct Metrics {
    pub pending_tasks: Gauge,

    pub message_processing_time: Histogram,
    pub pending_messages: Gauge,
    pub processed_messages: Counter,
}

impl Metrics {
    pub fn new() -> Self {
        let pending_tasks = Gauge::default();
        let message_processing_time = Histogram::new(exponential_buckets(
            0.001, // 1ms
            2.0, 16,
        ));
        let pending_messages = Gauge::default();
        let processed_messages = Counter::default();

        Self {
            pending_tasks,
            message_processing_time,
            pending_messages,
            processed_messages,
        }
    }

    pub fn register(&self, name: &str, registry: &mut Registry) {
        let sub_registry = registry
            .sub_registry_with_prefix("serviceless")
            .sub_registry_with_label(("service".into(), String::from(name).into()));

        sub_registry.register(
            "pending_tasks",
            "Number of pending tasks",
            self.pending_tasks.clone(),
        );

        sub_registry.register(
            "message_processing_time",
            "Time taken to process messages",
            self.message_processing_time.clone(),
        );

        sub_registry.register(
            "pending_messages",
            "Number of pending messages",
            self.pending_messages.clone(),
        );

        sub_registry.register(
            "processed_messages",
            "Number of processed messages",
            self.processed_messages.clone(),
        );
    }
}
