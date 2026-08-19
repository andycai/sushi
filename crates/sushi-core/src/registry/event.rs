use crate::runtime::{
    CapabilityRegistry, EventSubscriptionSpec, PluginInstanceId, RegistrationConflict,
};
use serde_json::Value;
use std::future::Future;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventSubscription {
    pub id: u64,
    pub owner: PluginInstanceId,
    pub event: String,
}

#[derive(Clone, Default)]
pub struct EventBus {
    registry: CapabilityRegistry,
}

impl EventBus {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn new_with_registry(registry: CapabilityRegistry) -> Self {
        Self { registry }
    }

    pub async fn on<F, Fut>(
        &self,
        event: &str,
        handler: F,
    ) -> Result<EventSubscription, RegistrationConflict>
    where
        F: Fn(&Value) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        self.on_owned(PluginInstanceId::legacy("event-bus"), event, handler)
            .await
    }

    pub async fn on_owned<F, Fut>(
        &self,
        owner: PluginInstanceId,
        event: &str,
        handler: F,
    ) -> Result<EventSubscription, RegistrationConflict>
    where
        F: Fn(&Value) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let spec = EventSubscriptionSpec::new(event, move |data| handler(&data));
        let subscription = EventSubscription {
            id: spec.subscription_id,
            owner: owner.clone(),
            event: spec.event.clone(),
        };
        let mut staged = self.registry.stage(owner);
        staged.register_event_subscription(spec);
        self.registry.commit(staged).await?;
        Ok(subscription)
    }

    pub async fn remove(&self, subscription: &EventSubscription) {
        self.registry
            .remove_event_subscription(subscription.id)
            .await;
    }

    pub async fn remove_owner(&self, owner: &PluginInstanceId) {
        self.registry
            .remove_event_subscriptions_by_owner(owner)
            .await;
    }

    pub async fn emit(&self, event: &str, data: &Value) {
        let handlers = self
            .registry
            .snapshot_sync()
            .event_subscriptions()
            .iter()
            .filter(|registration| registration.value.event == event)
            .map(|registration| registration.value.clone())
            .collect::<Vec<_>>();

        for handler in handlers {
            handler.call(data.clone()).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::HttpRouteSpec;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_subscribe_and_emit() {
        let bus = EventBus::new();
        let counter = Arc::new(AtomicUsize::new(0));

        let c = counter.clone();
        bus.on("test.event", move |data| {
            let c = c.clone();
            let v = data.get("value").and_then(|v| v.as_i64()).unwrap_or(0) as usize;
            async move {
                c.fetch_add(v, Ordering::SeqCst);
            }
        })
        .await
        .unwrap();

        bus.emit("test.event", &serde_json::json!({"value": 42}))
            .await;
        assert_eq!(counter.load(Ordering::SeqCst), 42);
    }

    #[tokio::test]
    async fn test_multiple_subscribers() {
        let bus = EventBus::new();
        let counter = Arc::new(AtomicUsize::new(0));

        let c1 = counter.clone();
        bus.on("multi", move |_| {
            let c = c1.clone();
            async move {
                c.fetch_add(1, Ordering::SeqCst);
            }
        })
        .await
        .unwrap();

        let c2 = counter.clone();
        bus.on("multi", move |_| {
            let c = c2.clone();
            async move {
                c.fetch_add(10, Ordering::SeqCst);
            }
        })
        .await
        .unwrap();

        bus.emit("multi", &Value::Null).await;
        assert_eq!(counter.load(Ordering::SeqCst), 11);
    }

    #[tokio::test]
    async fn owner_removal_keeps_other_subscribers() {
        let bus = EventBus::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let removed_owner = PluginInstanceId::new("notes.default").unwrap();
        let retained_owner = PluginInstanceId::new("cms.default").unwrap();

        let first_counter = Arc::clone(&counter);
        bus.on_owned(removed_owner.clone(), "notes.changed", move |_| {
            let counter = Arc::clone(&first_counter);
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
            }
        })
        .await
        .unwrap();
        let second_counter = Arc::clone(&counter);
        bus.on_owned(retained_owner, "notes.changed", move |_| {
            let counter = Arc::clone(&second_counter);
            async move {
                counter.fetch_add(10, Ordering::SeqCst);
            }
        })
        .await
        .unwrap();

        bus.remove_owner(&removed_owner).await;
        bus.emit("notes.changed", &Value::Null).await;
        assert_eq!(counter.load(Ordering::SeqCst), 10);
    }

    #[tokio::test]
    async fn event_owner_removal_does_not_remove_other_capabilities() {
        let registry = CapabilityRegistry::new();
        let owner = PluginInstanceId::new("notes.default").unwrap();
        let mut staged = registry.stage(owner.clone());
        staged.register_http(HttpRouteSpec::new(
            "GET",
            "/api/notes",
            "notes",
            "handler::list",
        ));
        registry.commit(staged).await.unwrap();
        let bus = EventBus::new_with_registry(registry.clone());
        bus.on_owned(owner.clone(), "notes.changed", |_| async {})
            .await
            .unwrap();

        bus.remove_owner(&owner).await;

        let snapshot = registry.snapshot().await;
        assert_eq!(snapshot.http_routes().len(), 1);
        assert!(snapshot.event_subscriptions().is_empty());
    }

    #[tokio::test]
    async fn emit_does_not_hold_registry_lock_during_callback() {
        let bus = EventBus::new();
        let nested_bus = bus.clone();
        bus.on("lock.test", move |_| {
            let nested_bus = nested_bus.clone();
            async move {
                nested_bus.on("nested", |_| async {}).await.unwrap();
            }
        })
        .await
        .unwrap();

        tokio::time::timeout(
            std::time::Duration::from_secs(1),
            bus.emit("lock.test", &Value::Null),
        )
        .await
        .expect("event callback must not wait on the registry lock");
    }

    #[tokio::test]
    async fn test_emit_no_subscribers() {
        EventBus::new().emit("nonexistent", &Value::Null).await;
    }
}
