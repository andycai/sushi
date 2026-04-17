use anyhow::{Context, Result};
use std::env;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use sushi_core::auth::jwt::JwtService;
use sushi_core::auth::policy_repository::PolicyRepository;
use sushi_core::config::{ConfigStore, SushiConfig};
use sushi_core::context::SushiContext;
use sushi_core::logs::tracing_bridge;
use sushi_core::lua::loader::LuaPlugin;
use sushi_core::plugin::Plugin;
use sushi_core::storage::sqlite::SqliteStorage;
use sushi_core::storage::Storage;
use sushi_core::web::template_service::TemplateService;

const MIGRATION_SQL: &str = include_str!("../../../migrations/001_init.sql");
const KV_MIGRATION_SQL: &str = include_str!("../../../migrations/002_kv_store.sql");
const RBAC_MIGRATION_SQL: &str = include_str!("../../../migrations/003_rbac.sql");
const MENU_MIGRATION_SQL: &str = include_str!("../../../migrations/004_menu.sql");
const MENUS_RBAC_MIGRATION_SQL: &str = include_str!("../../../migrations/005_menus_rbac.sql");
const UNIFIED_POLICY_V2_MIGRATION_SQL: &str =
    include_str!("../../../migrations/006_unified_policy_v2.sql");

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
    storage
        .run_migrations(UNIFIED_POLICY_V2_MIGRATION_SQL)
        .await
        .context("failed to run unified policy migrations")?;

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

    let file_browser_root_dir = {
        let guard = config.get().await;
        resolve_file_browser_root_dir(config_path, &guard.file_browser.root_dir)?
    };

    let plugins_dir = {
        let guard = config.get().await;
        PathBuf::from(&guard.plugins.directory)
    };

    config
        .update(|cfg| {
            cfg.web.static_dir = static_dir.to_string_lossy().to_string();
            cfg.file_browser.root_dir = file_browser_root_dir.to_string_lossy().to_string();
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

    let mut lua_plugins = Vec::new();
    let mut plugin_template_roots = Vec::new();
    let mut plugin_static_roots = Vec::new();
    if plugins_dir.exists() {
        lua_plugins = LuaPlugin::scan_dir(&plugins_dir)
            .await
            .context("failed to scan plugins directory")?;
        for plugin in &lua_plugins {
            let plugin_path_id = plugin.path_id().to_string();
            let template_root = plugin.web_templates_dir();
            if template_root.is_dir() {
                plugin_template_roots.push((plugin_path_id.clone(), template_root));
            }
            let static_root = plugin.web_static_dir();
            if static_root.is_dir() {
                plugin_static_roots.push((plugin_path_id, static_root));
            }
        }
    }

    let templates =
        TemplateService::new_with_plugin_roots(&templates_dir, plugin_template_roots)
            .with_context(|| format!("failed to init template root {}", templates_dir.display()))?;

    let ctx = SushiContext::new(config, storage, jwt, templates);
    tracing_bridge::register_log_service(ctx.logs.clone());

    for (plugin_name, static_root) in plugin_static_roots {
        ctx.plugins
            .register_plugin_static_root(&plugin_name, static_root)
            .await;
    }

    // Load plugins
    for plugin in lua_plugins {
        let plugin_name = plugin.name().to_string();
        ctx.plugins
            .register_plugin_manifest_with_permissions(
                plugin.manifest(),
                plugin.effective_permissions(),
            )
            .await;

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

    hydrate_authorizer_snapshot(&ctx).await?;

    Ok(ctx)
}

fn resolve_templates_dir(config_path: Option<&Path>, templates_dir: &str) -> Result<PathBuf> {
    resolve_dir(config_path, templates_dir, "template")
}

fn resolve_static_dir(config_path: Option<&Path>, static_dir: &str) -> Result<PathBuf> {
    resolve_dir(config_path, static_dir, "static")
}

fn resolve_file_browser_root_dir(config_path: Option<&Path>, root_dir: &str) -> Result<PathBuf> {
    resolve_dir(config_path, root_dir, "file-browser root")
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

async fn hydrate_authorizer_snapshot(ctx: &SushiContext) -> Result<()> {
    let storage: Arc<dyn Storage> = ctx.db.clone();
    let repository = PolicyRepository::new(storage);
    let snapshot = repository
        .compile_snapshot()
        .await
        .map_err(anyhow::Error::msg)
        .context("failed to compile policy snapshot from database")?;
    ctx.authorizer.replace_snapshot(snapshot).await;
    Ok(())
}
