use anyhow::{Context, Result};
use std::env;
use std::path::{Path, PathBuf};
use sushi_core::auth::jwt::JwtService;
use sushi_core::config::{ConfigStore, SushiConfig};
use sushi_core::context::SushiContext;
use sushi_core::lua::loader::LuaPlugin;
use sushi_core::plugin::Plugin;
use sushi_core::storage::sqlite::SqliteStorage;
use sushi_core::web::template_service::TemplateService;

const MIGRATION_SQL: &str = include_str!("../../../migrations/001_init.sql");
const KV_MIGRATION_SQL: &str = include_str!("../../../migrations/002_kv_store.sql");
const RBAC_MIGRATION_SQL: &str = include_str!("../../../migrations/003_rbac.sql");
const MENU_MIGRATION_SQL: &str = include_str!("../../../migrations/004_menu.sql");
const MENUS_RBAC_MIGRATION_SQL: &str = include_str!("../../../migrations/005_menus_rbac.sql");

pub async fn bootstrap(config_path: Option<&Path>) -> Result<SushiContext> {
    let config = match config_path {
        Some(path) if path.exists() => ConfigStore::load(path)
            .await
            .context("failed to load config")?,
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
    storage
        .run_migrations(MIGRATION_SQL)
        .await
        .context("failed to run migrations")?;
    storage
        .run_migrations(KV_MIGRATION_SQL)
        .await
        .context("failed to run kv migrations")?;
    storage
        .run_migrations(RBAC_MIGRATION_SQL)
        .await
        .context("failed to run rbac migrations")?;
    storage
        .run_migrations(MENU_MIGRATION_SQL)
        .await
        .context("failed to run menu migrations")?;
    storage
        .run_migrations(MENUS_RBAC_MIGRATION_SQL)
        .await
        .context("failed to run menus rbac migrations")?;

    let jwt = {
        let guard = config.get().await;
        JwtService::new(
            &guard.jwt.secret,
            guard.jwt.access_ttl,
            guard.jwt.refresh_ttl,
        )
    };

    let templates_dir = {
        let guard = config.get().await;
        resolve_templates_dir(config_path, &guard.web.templates_dir)?
    };

    let static_dir = {
        let guard = config.get().await;
        resolve_static_dir(config_path, &guard.web.static_dir)?
    };

    config
        .update(|cfg| {
            cfg.web.static_dir = static_dir.to_string_lossy().to_string();
        })
        .await;

    tokio::fs::create_dir_all(&templates_dir)
        .await
        .with_context(|| {
            format!(
                "failed to create templates directory {}",
                templates_dir.display()
            )
        })?;

    let templates = TemplateService::new(&templates_dir)
        .with_context(|| format!("failed to init template root {}", templates_dir.display()))?;

    let ctx = SushiContext::new(config, storage, jwt, templates);

    // Load plugins
    let plugins_dir = {
        let guard = ctx.config.get().await;
        PathBuf::from(&guard.plugins.directory)
    };
    if plugins_dir.exists() {
        let lua_plugins = LuaPlugin::scan_dir(&plugins_dir)
            .await
            .context("failed to scan plugins directory")?;
        for plugin in lua_plugins {
            let plugin_name = plugin.name().to_string();
            let manifest = plugin.manifest().clone();
            ctx.plugins.register_plugin_manifest(&manifest).await;

            if let Err(e) = plugin.init(&ctx).await {
                tracing::warn!("failed to init plugin {plugin_name}: {e}");
                ctx.logs
                    .warn(&format!("failed to init plugin {plugin_name}: {e}"))
                    .await;
                ctx.plugins.mark_plugin_loaded(&plugin_name, false).await;
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

fn resolve_templates_dir(config_path: Option<&Path>, templates_dir: &str) -> Result<PathBuf> {
    resolve_dir(config_path, templates_dir, "template")
}

fn resolve_static_dir(config_path: Option<&Path>, static_dir: &str) -> Result<PathBuf> {
    resolve_dir(config_path, static_dir, "static")
}

fn resolve_dir(config_path: Option<&Path>, dir: &str, kind: &str) -> Result<PathBuf> {
    let base_dir = match config_path.and_then(|path| path.parent()) {
        Some(parent) => parent.to_path_buf(),
        None => env::current_dir().with_context(|| {
            format!("failed to determine current working directory for {kind} resolution")
        })?,
    };

    let candidate = PathBuf::from(dir);
    if candidate.is_absolute() {
        Ok(candidate)
    } else {
        Ok(base_dir.join(candidate))
    }
}
