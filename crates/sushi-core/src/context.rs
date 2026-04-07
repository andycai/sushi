use crate::auth::jwt::JwtService;
use crate::auth::middleware::AuthState;
use crate::config::ConfigStore;
use crate::plugin::manager::PluginManager;
use crate::registry::event::EventBus;
use crate::storage::sqlite::SqliteStorage;
use std::sync::Arc;

/// The central context passed to all plugins during init.
/// Provides access to the plugin manager, event bus, and core services.
#[derive(Clone)]
pub struct SushiContext {
    pub config: ConfigStore,
    pub db: Arc<SqliteStorage>,
    pub event: EventBus,
    pub jwt: Arc<JwtService>,
    pub plugins: PluginManager,
}

impl SushiContext {
    /// Creates a new SushiContext from the given core services.
    pub fn new(config: ConfigStore, db: SqliteStorage, jwt: JwtService) -> Self {
        Self {
            config,
            db: Arc::new(db),
            event: EventBus::new(),
            jwt: Arc::new(jwt),
            plugins: PluginManager::new(),
        }
    }

    /// Returns an [`AuthState`] suitable for use as Axum middleware state.
    pub fn auth_state(&self) -> AuthState {
        AuthState {
            jwt_service: Arc::clone(&self.jwt),
        }
    }
}
