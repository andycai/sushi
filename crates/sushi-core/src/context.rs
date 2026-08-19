use crate::auth::authorizer::{Authorizer, CompiledPolicySnapshot};
use crate::auth::jwt::JwtService;
use crate::auth::middleware::AuthState;
use crate::auth::policy_repository::PolicyRepository;
use crate::config::ConfigStore;
use crate::db::{DbGateway, DbPermission};
use crate::logs::LogService;
use crate::lua::loader::RuntimeHost;
use crate::plugin::manager::PluginManager;
use crate::registry::event::EventBus;
use crate::runtime::{ResolvedRuntimeProfile, TaskRegistry};
use crate::storage::sqlite::SqliteStorage;
use crate::storage::Storage;
use crate::web::template_service::TemplateService;
use std::sync::Arc;
use std::time::Duration;

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
            ResolvedRuntimeProfile::legacy_empty(),
        )
    }

    pub fn new_with_runtime_profile(
        config: ConfigStore,
        db: SqliteStorage,
        jwt: JwtService,
        templates: TemplateService,
        runtime_profile: ResolvedRuntimeProfile,
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
            runtime_host: RuntimeHost::new(),
            runtime_profile: Arc::new(runtime_profile),
            templates: Arc::new(templates),
            tasks: TaskRegistry::new(),
            logs: Arc::new(LogService::new()),
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

    pub async fn set_plugin_enabled(
        &self,
        plugin_name: &str,
        enabled: bool,
        actor: Option<&str>,
        reason: Option<&str>,
    ) -> Result<crate::plugin::manager::PluginInfo, String> {
        if self.plugins.is_plugin_required(plugin_name).await {
            return Err(format!(
                "required_plugin_toggle_forbidden: plugin '{plugin_name}' must be changed through profile and restart"
            ));
        }

        let _runtime_guard = self.plugins.acquire_plugin_runtime_lock(plugin_name).await;
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
