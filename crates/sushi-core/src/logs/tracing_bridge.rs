use std::sync::{Arc, OnceLock, RwLock, Weak};

use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::Layer;

use super::LogService;

static ACTIVE_LOG_SERVICE: OnceLock<RwLock<Weak<LogService>>> = OnceLock::new();

fn active_log_service_cell() -> &'static RwLock<Weak<LogService>> {
    ACTIVE_LOG_SERVICE.get_or_init(|| RwLock::new(Weak::new()))
}

fn current_log_service() -> Option<Arc<LogService>> {
    let guard = active_log_service_cell().read().ok()?;
    guard.upgrade()
}

pub fn register_log_service(log_service: Arc<LogService>) {
    if let Ok(mut guard) = active_log_service_cell().write() {
        *guard = Arc::downgrade(&log_service);
    }
}

#[derive(Default)]
pub struct LogServiceBridgeLayer;

pub fn layer() -> LogServiceBridgeLayer {
    LogServiceBridgeLayer
}

#[derive(Default)]
struct EventVisitor {
    message: Option<String>,
    fields: Vec<String>,
}

impl EventVisitor {
    fn record_value(&mut self, field_name: &str, value: String) {
        if field_name == "message" {
            self.message = Some(value);
            return;
        }
        self.fields.push(format!("{field_name}={value}"));
    }

    fn into_message(self, target: &str) -> String {
        let mut parts = Vec::new();
        let trimmed_target = target.trim();
        if !trimmed_target.is_empty() {
            parts.push(format!("target={trimmed_target}"));
        }
        if let Some(message) = self.message {
            parts.push(message);
        }
        if !self.fields.is_empty() {
            parts.push(self.fields.join(" "));
        }
        parts.join(" | ")
    }
}

impl Visit for EventVisitor {
    fn record_i64(&mut self, field: &Field, value: i64) {
        self.record_value(field.name(), value.to_string());
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.record_value(field.name(), value.to_string());
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.record_value(field.name(), value.to_string());
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.record_value(field.name(), value.to_string());
    }

    fn record_error(&mut self, field: &Field, value: &(dyn std::error::Error + 'static)) {
        self.record_value(field.name(), value.to_string());
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.record_value(field.name(), format!("{value:?}"));
    }
}

impl<S> Layer<S> for LogServiceBridgeLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let level = *event.metadata().level();
        if !matches!(level, Level::WARN | Level::ERROR) {
            return;
        }

        let log_service = match current_log_service() {
            Some(service) => service,
            None => return,
        };

        let mut visitor = EventVisitor::default();
        event.record(&mut visitor);
        let message = visitor.into_message(event.metadata().target());
        if message.is_empty() {
            return;
        }

        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            return;
        };

        handle.spawn(async move {
            match level {
                Level::WARN => log_service.warn(&message).await,
                Level::ERROR => log_service.error(&message).await,
                _ => {}
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing_subscriber::prelude::*;

    #[tokio::test]
    async fn bridge_layer_collects_warn_and_error_only() {
        let logs = Arc::new(LogService::new());
        register_log_service(logs.clone());

        let subscriber = tracing_subscriber::registry().with(layer());
        tracing::subscriber::with_default(subscriber, || {
            tracing::warn!("warn message");
            tracing::error!(reason = "broken", "error message");
            tracing::info!("info should not be captured");
        });

        // Wait briefly for spawned append tasks to complete.
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;

        let entries = logs.list(20).await;
        assert!(
            entries.iter().any(|entry| entry.level == "WARN"),
            "entries: {entries:?}"
        );
        assert!(
            entries.iter().any(|entry| entry.level == "ERROR"),
            "entries: {entries:?}"
        );
        assert!(
            !entries.iter().any(|entry| entry.level == "INFO"),
            "entries: {entries:?}"
        );
    }
}
