use crate::auth::jwt::JwtService;
use crate::auth::middleware::AuthState;
use crate::config::ConfigStore;
use crate::db::{DbGateway, DbPermission};
use crate::logs::LogService;
use crate::plugin::manager::PluginManager;
use crate::registry::event::EventBus;
use crate::storage::Storage;
use crate::storage::sqlite::SqliteStorage;
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
        let db_gateway = DbGateway::new(storage, DbPermission::Admin);

        Self {
            config,
            db,
            db_gateway,
            event: EventBus::new(),
            jwt: Arc::new(jwt),
            plugins: PluginManager::new(),
            templates: Arc::new(templates),
            logs: Arc::new(LogService::new()),
        }
    }

    /// Returns an [`AuthState`] suitable for use as Axum middleware state.
    pub fn auth_state(&self) -> AuthState {
        AuthState {
            jwt_service: Arc::clone(&self.jwt),
        }
    }
}
