use crate::auth::authorizer::{Authorizer, CompiledPolicySnapshot};
use crate::auth::jwt::JwtService;
use crate::auth::middleware::AuthState;
use crate::auth::policy_repository::PolicyRepository;
use crate::config::ConfigStore;
use crate::db::{DbGateway, DbPermission};
use crate::logs::LogService;
use crate::plugin::manager::PluginManager;
use crate::registry::event::EventBus;
use crate::storage::sqlite::SqliteStorage;
use crate::storage::Storage;
use crate::web::template_service::TemplateService;
use std::sync::Arc;

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
    pub templates: Arc<TemplateService>,
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
        let db = Arc::new(db);
        let storage: Arc<dyn Storage> = db.clone();
        let db_gateway = DbGateway::new(storage.clone(), DbPermission::Admin);

        Self {
            config,
            db,
            db_gateway,
            event: EventBus::new(),
            jwt: Arc::new(jwt),
            authorizer: Arc::new(Authorizer::new(CompiledPolicySnapshot::default())),
            plugins: PluginManager::new_with_storage(storage),
            templates: Arc::new(templates),
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
}
