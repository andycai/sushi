use crate::auth::authorizer::{Authorizer, CompiledPolicySnapshot};
use crate::auth::jwt::JwtService;
use crate::auth::middleware::AuthState;
use crate::auth::policy_repository::PolicyRepository;
use crate::config::ConfigStore;
use crate::db::{DbGateway, DbPermission};
use crate::logs::LogService;
use crate::lua::loader::RuntimeHost;
use crate::plugin::manager::PluginManager;
use crate::plugin::{DatabasePermission, Permissions};
use crate::registry::event::{EventBus, EventSubscription};
use crate::runtime::RegistrationConflict;
use crate::runtime::{
    PendingTask, PluginCancellationToken, PluginInstanceId, ResolvedRuntimeProfile,
    TaskRegistration, TaskRegistry,
};
use crate::storage::sqlite::SqliteStorage;
use crate::storage::Storage;
use crate::web::template_service::TemplateService;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct PluginContext {
    instance: PluginInstanceId,
    config: ConfigStore,
    config_value: Value,
    permissions: Permissions,
    db_gateway: Option<DbGateway>,
    event: PluginEventBus,
    jwt: Arc<JwtService>,
    templates: Arc<TemplateService>,
    logs: Arc<LogService>,
    cancellation: PluginCancellationToken,
    pending_tasks: Arc<Mutex<Vec<PendingTask>>>,
    pub(crate) host: Arc<PluginHostServices>,
}

pub(crate) struct PluginHostServices {
    pub(crate) db: Arc<SqliteStorage>,
    pub(crate) plugins: PluginManager,
    pub(crate) tasks: TaskRegistry,
}

#[derive(Clone)]
pub struct PluginEventBus {
    bus: EventBus,
    owner: PluginInstanceId,
}

impl PluginEventBus {
    pub async fn on<F, Fut>(
        &self,
        event: &str,
        handler: F,
    ) -> Result<EventSubscription, RegistrationConflict>
    where
        F: Fn(&Value) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        self.bus.on_owned(self.owner.clone(), event, handler).await
    }

    pub async fn emit(&self, event: &str, data: &Value) {
        self.bus.emit(event, data).await;
    }
}

impl PluginContext {
    pub fn instance(&self) -> &PluginInstanceId {
        &self.instance
    }

    pub fn permissions(&self) -> &Permissions {
        &self.permissions
    }

    pub(crate) fn config(&self) -> &ConfigStore {
        &self.config
    }

    pub fn config_value(&self) -> &Value {
        &self.config_value
    }

    pub fn db(&self) -> Option<DbGateway> {
        self.db_gateway.clone()
    }

    pub fn event(&self) -> PluginEventBus {
        self.event.clone()
    }

    pub fn jwt(&self) -> Arc<JwtService> {
        Arc::clone(&self.jwt)
    }

    pub fn templates(&self) -> Arc<TemplateService> {
        Arc::clone(&self.templates)
    }

    pub fn logs(&self) -> Arc<LogService> {
        Arc::clone(&self.logs)
    }

    pub fn cancellation(&self) -> PluginCancellationToken {
        self.cancellation.clone()
    }

    pub async fn register_task<F, Fut>(
        &self,
        name: impl Into<String>,
        task: F,
    ) -> Result<(), String>
    where
        F: FnOnce(crate::runtime::TaskCancellationToken) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        let name = name.into();
        if name.trim().is_empty() || name.chars().any(char::is_control) {
            return Err(
                "background task name must be non-empty and contain no control characters"
                    .to_string(),
            );
        }
        let mut pending = self.pending_tasks.lock().await;
        if pending.iter().any(|task| task.name == name) {
            return Err(format!("duplicate background task name '{name}'"));
        }
        pending.push(PendingTask::new(name, task));
        Ok(())
    }

    pub(crate) async fn start_registered_tasks(&self) -> Vec<TaskRegistration> {
        let pending = std::mem::take(&mut *self.pending_tasks.lock().await);
        let mut registrations = Vec::with_capacity(pending.len());
        for task in pending {
            registrations.push(
                self.host
                    .tasks
                    .start_pending(self.instance.clone(), task)
                    .await,
            );
        }
        registrations
    }

    pub(crate) fn storage(&self) -> &SqliteStorage {
        &self.host.db
    }

    pub(crate) fn plugin_manager(&self) -> &PluginManager {
        &self.host.plugins
    }
}

/// The central context passed to all plugins during init.
/// Provides access to the plugin manager, event bus, and core services.
#[derive(Clone)]
pub struct SushiContext {
    pub config: ConfigStore,
    pub db: Arc<SqliteStorage>,
    pub db_gateway: DbGateway,
    pub event: EventBus,
    pub jwt: Arc<JwtService>,
    pub authorizer: Arc<Authorizer>,
    pub plugins: PluginManager,
    pub runtime_host: RuntimeHost,
    pub runtime_profile: Arc<ResolvedRuntimeProfile>,
    pub templates: Arc<TemplateService>,
    pub(crate) tasks: TaskRegistry,
    pub logs: Arc<LogService>,
}

impl SushiContext {
    /// Creates a new SushiContext from the given core services.
    pub fn new(
        config: ConfigStore,
        db: SqliteStorage,
        jwt: JwtService,
        templates: TemplateService,
    ) -> Self {
        Self::new_with_runtime_profile(
            config,
            db,
            jwt,
            templates,
            ResolvedRuntimeProfile::empty_for_host(),
        )
    }

    pub fn new_with_runtime_profile(
        config: ConfigStore,
        db: SqliteStorage,
        jwt: JwtService,
        templates: TemplateService,
        runtime_profile: ResolvedRuntimeProfile,
    ) -> Self {
        Self::new_with_runtime_profile_and_host(
            config,
            db,
            jwt,
            templates,
            runtime_profile,
            RuntimeHost::new(),
        )
    }

    pub fn new_with_runtime_profile_and_host(
        config: ConfigStore,
        db: SqliteStorage,
        jwt: JwtService,
        templates: TemplateService,
        runtime_profile: ResolvedRuntimeProfile,
        runtime_host: RuntimeHost,
    ) -> Self {
        let db = Arc::new(db);
        let storage: Arc<dyn Storage> = db.clone();
        let db_gateway = DbGateway::new(storage.clone(), DbPermission::Admin);
        let plugins = PluginManager::new_with_sqlite_storage(db.clone());
        templates.bind_registry(plugins.capability_registry());
        let event = EventBus::new_with_registry(plugins.capability_registry());

        Self {
            config,
            db,
            db_gateway,
            event,
            jwt: Arc::new(jwt),
            authorizer: Arc::new(Authorizer::new(CompiledPolicySnapshot::default())),
            plugins,
            runtime_host,
            runtime_profile: Arc::new(runtime_profile),
            templates: Arc::new(templates),
            tasks: TaskRegistry::new(),
            logs: Arc::new(LogService::new()),
        }
    }

    pub fn plugin_context(&self, permissions: &Permissions) -> PluginContext {
        self.plugin_context_for(
            PluginInstanceId::legacy("plugin"),
            Value::Object(serde_json::Map::new()),
            permissions,
        )
    }

    pub(crate) fn plugin_context_for(
        &self,
        instance: PluginInstanceId,
        config_value: Value,
        permissions: &Permissions,
    ) -> PluginContext {
        let database_permission = match &permissions.database {
            DatabasePermission::None => None,
            DatabasePermission::ReadOnly => Some(DbPermission::ReadOnly),
            DatabasePermission::Write => Some(DbPermission::Write),
            DatabasePermission::Admin => Some(DbPermission::Admin),
        };

        PluginContext {
            instance: instance.clone(),
            config: self.config.clone(),
            config_value,
            permissions: permissions.clone(),
            db_gateway: database_permission
                .map(|permission| self.db_gateway.with_permission(permission)),
            event: PluginEventBus {
                bus: self.event.clone(),
                owner: instance.clone(),
            },
            jwt: Arc::clone(&self.jwt),
            templates: Arc::clone(&self.templates),
            logs: Arc::clone(&self.logs),
            cancellation: PluginCancellationToken::new(),
            pending_tasks: Arc::new(Mutex::new(Vec::new())),
            host: Arc::new(PluginHostServices {
                db: Arc::clone(&self.db),
                plugins: self.plugins.clone(),
                tasks: self.tasks.clone(),
            }),
        }
    }

    /// Returns an [`AuthState`] suitable for use as Axum middleware state.
    pub fn auth_state(&self) -> AuthState {
        AuthState {
            jwt_service: Arc::clone(&self.jwt),
            authorizer: Arc::clone(&self.authorizer),
        }
    }

    /// Rebuild the in-memory authorizer snapshot from persisted policy data.
    pub async fn refresh_authorizer_snapshot(&self) -> Result<(), String> {
        let storage: Arc<dyn Storage> = self.db.clone();
        let repository = PolicyRepository::new(storage);
        let snapshot = repository.compile_snapshot().await?;
        self.authorizer.replace_snapshot(snapshot).await;
        Ok(())
    }

    pub async fn remove_owner_effects(&self, owner: &crate::runtime::PluginInstanceId) {
        self.plugins.remove_owner_capabilities(owner).await;
        self.tasks.cancel_owner(owner, Duration::from_secs(5)).await;
    }

    pub async fn shutdown(&self) {
        self.tasks.cancel_all(Duration::from_secs(5)).await;
    }

    pub async fn set_plugin_enabled(
        &self,
        plugin_name: &str,
        enabled: bool,
        actor: Option<&str>,
        reason: Option<&str>,
    ) -> Result<crate::plugin::manager::PluginInfo, String> {
        let _lifecycle_guard = self.runtime_host.acquire_lifecycle_lock(plugin_name).await;
        if self.plugins.is_plugin_required(plugin_name).await {
            return Err(format!(
                "required_plugin_toggle_forbidden: plugin '{plugin_name}' must be changed through profile and restart"
            ));
        }

        let current = self
            .plugins
            .list_plugins()
            .await
            .into_iter()
            .find(|plugin| plugin.name == plugin_name)
            .ok_or_else(|| format!("plugin not found: {plugin_name}"))?;
        if current.enabled == enabled && current.loaded == enabled {
            return Ok(current);
        }

        let intent = if current.enabled == enabled {
            current
        } else {
            self.plugins
                .set_plugin_enabled_intent(plugin_name, enabled, actor, reason)
                .await?
        };
        if enabled {
            if let Err(error) = self.runtime_host.activate_locked(self, plugin_name).await {
                self.plugins.mark_plugin_loaded(plugin_name, false).await;
                return Err(error.to_string());
            }
        } else {
            self.runtime_host
                .deactivate_locked(self, plugin_name)
                .await
                .map_err(|error| error.to_string())?;
        }

        Ok(self
            .plugins
            .list_plugins()
            .await
            .into_iter()
            .find(|plugin| plugin.name == plugin_name)
            .unwrap_or(intent))
    }
}
