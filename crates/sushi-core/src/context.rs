use crate::auth::jwt::JwtService;
use crate::auth::middleware::AuthState;
use crate::config::ConfigStore;
use crate::registry::event::EventBus;
use crate::registry::{AdminRegistry, ApiRegistry, CliRegistry};
use crate::storage::sqlite::SqliteStorage;
use std::sync::Arc;

/// The central context passed to all plugins during init.
/// Provides access to all registries and services.
#[derive(Clone)]
pub struct SushiContext {
    pub api: ApiRegistry,
    pub admin: AdminRegistry,
    pub cli: CliRegistry,
    pub config: ConfigStore,
    pub db: Arc<SqliteStorage>,
    pub event: EventBus,
    pub jwt: Arc<JwtService>,
}

impl SushiContext {
    /// Creates a new SushiContext from the given core services.
    /// Registries and event bus are initialised empty.
    pub fn new(config: ConfigStore, db: SqliteStorage, jwt: JwtService) -> Self {
        Self {
            api: ApiRegistry::new(),
            admin: AdminRegistry::new(),
            cli: CliRegistry::new(),
            config,
            db: Arc::new(db),
            event: EventBus::new(),
            jwt: Arc::new(jwt),
        }
    }

    /// Returns an [`AuthState`] suitable for use as Axum middleware state.
    pub fn auth_state(&self) -> AuthState {
        AuthState {
            jwt_service: Arc::clone(&self.jwt),
        }
    }
}
