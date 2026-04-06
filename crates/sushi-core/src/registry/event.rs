use serde_json::Value;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::RwLock;

type EventHandler = Box<dyn Fn(&Value) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

pub struct EventBus {
    subscribers: Arc<RwLock<HashMap<String, Vec<EventHandler>>>>,
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            subscribers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn on<F, Fut>(&self, event: &str, handler: F)
    where
        F: Fn(&Value) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let wrapped: EventHandler = Box::new(move |data| Box::pin(handler(data)));
        let mut subs = self.subscribers.write().await;
        subs.entry(event.to_string()).or_default().push(wrapped);
    }

    pub async fn emit(&self, event: &str, data: &Value) {
        let subs = self.subscribers.read().await;
        if let Some(handlers) = subs.get(event) {
            for handler in handlers {
                handler(data).await;
            }
        }
    }
}

impl Clone for EventBus {
    fn clone(&self) -> Self {
        Self {
            subscribers: Arc::clone(&self.subscribers),
        }
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn test_subscribe_and_emit() {
        let bus = EventBus::new();
        let counter = Arc::new(AtomicUsize::new(0));

        let c = counter.clone();
        bus.on("test.event", move |data| {
            let c = c.clone();
            let v = data
                .get("value")
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as usize;
            Box::pin(async move {
                c.fetch_add(v, Ordering::SeqCst);
            })
        })
        .await;

        let mut data = serde_json::Map::new();
        data.insert("value".to_string(), serde_json::json!(42));
        bus.emit("test.event", &serde_json::Value::Object(data)).await;

        assert_eq!(counter.load(Ordering::SeqCst), 42);
    }

    #[tokio::test]
    async fn test_multiple_subscribers() {
        let bus = EventBus::new();
        let counter = Arc::new(AtomicUsize::new(0));

        let c1 = counter.clone();
        bus.on("multi", move |_| {
            let c = c1.clone();
            Box::pin(async move { c.fetch_add(1, Ordering::SeqCst); })
        })
        .await;

        let c2 = counter.clone();
        bus.on("multi", move |_| {
            let c = c2.clone();
            Box::pin(async move { c.fetch_add(10, Ordering::SeqCst); })
        })
        .await;

        bus.emit("multi", &serde_json::Value::Null).await;
        assert_eq!(counter.load(Ordering::SeqCst), 11);
    }

    #[tokio::test]
    async fn test_emit_no_subscribers() {
        let bus = EventBus::new();
        // Should not panic
        bus.emit("nonexistent", &serde_json::Value::Null).await;
    }
}
