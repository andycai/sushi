use crate::context::{PluginContext, SushiContext};
use async_trait::async_trait;
use std::collections::BTreeMap;
use std::sync::Arc;

use super::{PluginMigration, ResolvedRuntimeEntry};

#[async_trait]
pub trait BuiltinPluginFactory: Send + Sync {
    fn key(&self) -> &'static str;

    fn migrations(
        &self,
        _entry: &ResolvedRuntimeEntry,
    ) -> Result<Vec<PluginMigration>, super::MigrationError> {
        Ok(Vec::new())
    }

    async fn activate(
        &self,
        ctx: &SushiContext,
        plugin_ctx: &PluginContext,
        entry: &ResolvedRuntimeEntry,
    ) -> anyhow::Result<()>;
}

#[derive(Clone, Default)]
pub struct BuiltinFactoryRegistry {
    factories: BTreeMap<String, Arc<dyn BuiltinPluginFactory>>,
}

impl BuiltinFactoryRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<F>(&mut self, factory: F) -> anyhow::Result<()>
    where
        F: BuiltinPluginFactory + 'static,
    {
        let key = factory.key().trim();
        if key.is_empty() {
            anyhow::bail!("builtin factory key must not be empty");
        }
        if self.factories.contains_key(key) {
            anyhow::bail!("duplicate builtin factory key '{key}'");
        }
        self.factories.insert(key.to_string(), Arc::new(factory));
        Ok(())
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.factories.keys().map(String::as_str)
    }

    pub async fn activate(
        &self,
        key: &str,
        ctx: &SushiContext,
        entry: &ResolvedRuntimeEntry,
    ) -> anyhow::Result<()> {
        let factory = self
            .factories
            .get(key)
            .ok_or_else(|| anyhow::anyhow!("unknown builtin plugin factory '{key}'"))?;
        let plugin_ctx = ctx.plugin_context_for(
            entry.id.clone(),
            entry.config.clone(),
            &crate::plugin::Permissions::default(),
        );
        factory.activate(ctx, &plugin_ctx, entry).await?;
        plugin_ctx.start_registered_tasks().await;
        Ok(())
    }

    pub fn migrations_for_entries(
        &self,
        entries: &[ResolvedRuntimeEntry],
    ) -> anyhow::Result<Vec<PluginMigration>> {
        let mut migrations = Vec::new();
        for entry in entries.iter().filter(|entry| entry.enabled) {
            let Some(key) = entry.source.builtin_key() else {
                continue;
            };
            let factory = self
                .factories
                .get(key)
                .ok_or_else(|| anyhow::anyhow!("unknown builtin plugin factory '{key}'"))?;
            migrations.extend(factory.migrations(entry).map_err(|error| {
                anyhow::anyhow!(
                    "failed to build migrations for builtin entry '{}': {error}",
                    entry.id
                )
            })?);
        }
        Ok(migrations)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::jwt::JwtService;
    use crate::config::{ConfigStore, SushiConfig};
    use crate::runtime::{MigrationError, PluginInstanceId, RuntimePluginSource};
    use crate::storage::sqlite::SqliteStorage;
    use crate::web::template_service::TemplateService;
    use std::sync::atomic::{AtomicBool, Ordering};

    async fn test_context() -> (SushiContext, tempfile::TempDir) {
        let config = ConfigStore::new(SushiConfig::default());
        let db = SqliteStorage::new_in_memory().await.unwrap();
        let jwt = JwtService::new("test-secret-key-at-least-32-chars-long!", 3600, 604800);
        let templates_dir = tempfile::tempdir().unwrap();
        let templates = TemplateService::new(templates_dir.path()).unwrap();
        (SushiContext::new(config, db, jwt, templates), templates_dir)
    }

    struct DuplicateFactory;

    #[async_trait]
    impl BuiltinPluginFactory for DuplicateFactory {
        fn key(&self) -> &'static str {
            "duplicate"
        }

        async fn activate(
            &self,
            _ctx: &SushiContext,
            _plugin_ctx: &PluginContext,
            _entry: &ResolvedRuntimeEntry,
        ) -> anyhow::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn duplicate_factory_keys_fail_closed() {
        let mut registry = BuiltinFactoryRegistry::new();
        registry.register(DuplicateFactory).unwrap();
        assert!(registry.register(DuplicateFactory).is_err());
    }

    struct MigrationFactory;

    #[async_trait]
    impl BuiltinPluginFactory for MigrationFactory {
        fn key(&self) -> &'static str {
            "migration"
        }

        fn migrations(
            &self,
            _entry: &ResolvedRuntimeEntry,
        ) -> Result<Vec<PluginMigration>, MigrationError> {
            Ok(vec![PluginMigration::new(
                "builtin/migration",
                "001_probe",
                "CREATE TABLE probe (id INTEGER PRIMARY KEY);",
            )
            .unwrap()])
        }

        async fn activate(
            &self,
            _ctx: &SushiContext,
            _plugin_ctx: &PluginContext,
            _entry: &ResolvedRuntimeEntry,
        ) -> anyhow::Result<()> {
            Ok(())
        }
    }

    fn migration_entry(enabled: bool) -> ResolvedRuntimeEntry {
        ResolvedRuntimeEntry {
            id: PluginInstanceId::new("migration.default").unwrap(),
            source: RuntimePluginSource::Builtin {
                key: "migration".to_string(),
                reference: "builtin:migration".to_string(),
            },
            enabled,
            required: false,
            config: serde_json::json!({}),
            grants: serde_json::json!({}),
            origin: "test".to_string(),
        }
    }

    #[test]
    fn migration_catalog_comes_only_from_enabled_profile_factories() {
        let mut registry = BuiltinFactoryRegistry::new();
        registry.register(MigrationFactory).unwrap();

        let migrations = registry
            .migrations_for_entries(&[migration_entry(true), migration_entry(false)])
            .unwrap();

        assert_eq!(migrations.len(), 1);
        assert_eq!(migrations[0].plugin_id(), "builtin/migration");
        assert_eq!(migrations[0].migration_id(), "001_probe");
    }

    struct TaskFactory {
        cancelled: Arc<AtomicBool>,
        fail: bool,
    }

    #[async_trait]
    impl BuiltinPluginFactory for TaskFactory {
        fn key(&self) -> &'static str {
            "task"
        }

        async fn activate(
            &self,
            _ctx: &SushiContext,
            plugin_ctx: &PluginContext,
            _entry: &ResolvedRuntimeEntry,
        ) -> anyhow::Result<()> {
            let cancelled = Arc::clone(&self.cancelled);
            plugin_ctx
                .register_task("worker", move |mut token| async move {
                    token.cancelled().await;
                    cancelled.store(true, Ordering::SeqCst);
                })
                .await
                .map_err(anyhow::Error::msg)?;
            if self.fail {
                anyhow::bail!("activation failed after task registration");
            }
            Ok(())
        }
    }

    fn task_entry() -> ResolvedRuntimeEntry {
        ResolvedRuntimeEntry {
            id: PluginInstanceId::new("task.default").unwrap(),
            source: RuntimePluginSource::Builtin {
                key: "task".to_string(),
                reference: "builtin:task".to_string(),
            },
            enabled: true,
            required: false,
            config: serde_json::json!({}),
            grants: serde_json::json!({}),
            origin: "test".to_string(),
        }
    }

    #[tokio::test]
    async fn rust_builtin_tasks_start_only_after_successful_activation() {
        let cancelled = Arc::new(AtomicBool::new(false));
        let mut registry = BuiltinFactoryRegistry::new();
        registry
            .register(TaskFactory {
                cancelled: Arc::clone(&cancelled),
                fail: false,
            })
            .unwrap();
        let (ctx, _templates_dir) = test_context().await;

        registry
            .activate("task", &ctx, &task_entry())
            .await
            .unwrap();
        assert_eq!(ctx.tasks.active_count(&task_entry().id).await, 1);

        ctx.shutdown().await;
        assert!(cancelled.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn failed_rust_builtin_activation_starts_no_task() {
        let cancelled = Arc::new(AtomicBool::new(false));
        let mut registry = BuiltinFactoryRegistry::new();
        registry
            .register(TaskFactory {
                cancelled: Arc::clone(&cancelled),
                fail: true,
            })
            .unwrap();
        let (ctx, _templates_dir) = test_context().await;
        let entry = task_entry();

        let error = registry.activate("task", &ctx, &entry).await.unwrap_err();
        assert!(error.to_string().contains("activation failed"));
        assert_eq!(ctx.tasks.active_count(&entry.id).await, 0);
        assert!(!cancelled.load(Ordering::SeqCst));
    }
}
