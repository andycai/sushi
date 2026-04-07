use anyhow::{Context, Result};
use sushi_core::auth::jwt::JwtService;
use sushi_core::config::{ConfigStore, SushiConfig};
use sushi_core::context::SushiContext;
use sushi_core::lua::loader::LuaPlugin;
use sushi_core::plugin::Plugin;
use sushi_core::storage::sqlite::SqliteStorage;
use std::path::Path;

const MIGRATION_SQL: &str = include_str!("../../../migrations/001_init.sql");
const KV_MIGRATION_SQL: &str = include_str!("../../../migrations/002_kv_store.sql");

pub async fn bootstrap(config_path: Option<&Path>) -> Result<SushiContext> {
    let config = match config_path {
        Some(path) if path.exists() => {
            ConfigStore::load(path).await.context("failed to load config")?
        }
        Some(path) => {
            tracing::info!("no config file found at {}, using defaults", path.display());
            ConfigStore::new(SushiConfig::default())
        }
        None => ConfigStore::new(SushiConfig::default()),
    };

    let db_path = {
        let guard = config.get().await;
        guard.database.path.clone()
    };
    
    if let Some(parent) = Path::new(&db_path).parent() {
        tokio::fs::create_dir_all(parent).await.ok();
    }
    
    let storage = SqliteStorage::new(&db_path)
        .await
        .context("failed to open database")?;
    storage.run_migrations(MIGRATION_SQL)
        .await
        .context("failed to run migrations")?;
    storage.run_migrations(KV_MIGRATION_SQL)
        .await
        .context("failed to run kv migrations")?;

    let jwt = {
        let guard = config.get().await;
        JwtService::new(&guard.jwt.secret, guard.jwt.access_ttl, guard.jwt.refresh_ttl)
    };
    
    let ctx = SushiContext::new(config, storage, jwt);

    // Load plugins
    let plugins_dir = {
        let guard = ctx.config.get().await;
        std::path::PathBuf::from(&guard.plugins.directory)
    };
    if plugins_dir.exists() {
        let lua_plugins = LuaPlugin::scan_dir(&plugins_dir)
            .await
            .context("failed to scan plugins directory")?;
        for plugin in lua_plugins {
            let plugin_name = plugin.name().to_string();
            if let Err(e) = plugin.init(&ctx).await {
                tracing::warn!("failed to init plugin {plugin_name}: {e}");
                continue;
            }
            if let Some(lua) = plugin.into_vm() {
                ctx.plugins.register_vm(&plugin_name, lua).await;
                tracing::debug!("registered VM for plugin {plugin_name}");
            }
        }
    }

    Ok(ctx)
}
