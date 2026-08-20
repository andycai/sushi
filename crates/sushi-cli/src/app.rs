use anyhow::{Context, Result};
use std::collections::BTreeSet;
use std::env;
use std::path::{Path, PathBuf};
use sushi_core::auth::jwt::JwtService;
use sushi_core::config::{ConfigStore, SushiConfig};
use sushi_core::context::SushiContext;
use sushi_core::logs::tracing_bridge;
use sushi_core::lua::loader::{LuaPlugin, RuntimeHost};
use sushi_core::plugin::Plugin;
use sushi_core::runtime::{
    load_lua_migrations, BuiltinFactoryRegistry, MigrationRunner, PluginMigration,
    ResolvedRuntimeProfile, RuntimePluginSource, RuntimeProfileResolver,
};
use sushi_core::storage::sqlite::SqliteStorage;
use sushi_core::web::template_service::TemplateService;

pub async fn bootstrap(config_path: Option<&Path>) -> Result<SushiContext> {
    bootstrap_with_profile(config_path, None).await
}

pub async fn resolve_runtime_profile(
    config_path: Option<&Path>,
    profile_override: Option<&str>,
) -> Result<(ConfigStore, ResolvedRuntimeProfile)> {
    resolve_runtime_profile_with_overlays(config_path, profile_override, &[]).await
}

pub async fn resolve_runtime_profile_with_overlays(
    config_path: Option<&Path>,
    profile_override: Option<&str>,
    overlay_paths: &[PathBuf],
) -> Result<(ConfigStore, ResolvedRuntimeProfile)> {
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

    let (configured_profile, profiles_dir, bundles_dir, plugins_dir) = {
        let guard = config.get().await;
        (
            profile_override
                .map(str::to_string)
                .or_else(|| guard.runtime.profile.clone()),
            resolve_dir(config_path, &guard.runtime.profiles_dir, "profile")?,
            resolve_dir(config_path, &guard.runtime.bundles_dir, "bundle")?,
            resolve_dir(config_path, &guard.plugins.directory, "plugin")?,
        )
    };
    let builtin_factories = builtin_factories()?;
    let resolver = RuntimeProfileResolver::new(profiles_dir, bundles_dir, plugins_dir)
        .with_builtins(builtin_factories.keys());
    let overlay_paths = overlay_paths.to_vec();
    let profile = tokio::task::spawn_blocking(move || {
        resolver.resolve_configured_with_overlays(configured_profile.as_deref(), &overlay_paths)
    })
    .await
    .context("runtime profile resolver task failed")?
    .context("failed to resolve runtime profile")?;
    Ok((config, profile))
}

pub async fn bootstrap_with_profile(
    config_path: Option<&Path>,
    profile_override: Option<&str>,
) -> Result<SushiContext> {
    bootstrap_with_profile_and_overlays(config_path, profile_override, &[]).await
}

pub async fn bootstrap_with_profile_and_overlays(
    config_path: Option<&Path>,
    profile_override: Option<&str>,
    overlay_paths: &[PathBuf],
) -> Result<SushiContext> {
    bootstrap_with_options(config_path, profile_override, overlay_paths, "admin").await
}

pub(crate) async fn bootstrap_with_options(
    config_path: Option<&Path>,
    profile_override: Option<&str>,
    overlay_paths: &[PathBuf],
    cli_role: &str,
) -> Result<SushiContext> {
    let (config, runtime_profile) =
        resolve_runtime_profile_with_overlays(config_path, profile_override, overlay_paths).await?;
    let mut resolved_lua_plugins = Vec::new();
    let mut plugin_names = BTreeSet::new();
    for entry in runtime_profile.entries() {
        let RuntimePluginSource::Lua { path_id, path, .. } = &entry.source else {
            continue;
        };
        let mut plugin = LuaPlugin::load_dir(path, path_id)
            .await
            .with_context(|| format!("failed to load profile entry {}", entry.id))?;
        plugin.apply_profile_grants(&entry.grants);
        if entry.enabled && entry.required && !plugin.is_approved() {
            anyhow::bail!(
                "required runtime entry {} source '{}' is not approved; set grants.approved = true and restart",
                entry.id,
                entry.source.reference()
            );
        }
        if !plugin_names.insert(plugin.name().to_string()) {
            anyhow::bail!(
                "runtime profile '{}' mounts duplicate plugin name '{}'",
                runtime_profile.name(),
                plugin.name()
            );
        }
        resolved_lua_plugins.push((entry.clone(), plugin));
    }

    let runtime_host = RuntimeHost::new();
    for (entry, plugin) in &resolved_lua_plugins {
        runtime_host
            .register_lua_source_for_instance_with_config(
                plugin,
                entry.id.clone(),
                entry.required,
                entry.config.clone(),
            )
            .await;
    }

    let migrating_plugins = resolved_lua_plugins
        .iter()
        .filter(|(entry, plugin)| entry.enabled && plugin.is_approved())
        .map(|(_, plugin)| plugin.name().to_string())
        .collect::<Vec<_>>();
    for plugin_name in &migrating_plugins {
        runtime_host
            .begin_migration(plugin_name)
            .await
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    }

    let builtin_factories = builtin_factories_with_role(cli_role, config_path)?;
    let mut migrations = match builtin_factories.migrations_for_entries(runtime_profile.entries()) {
        Ok(migrations) => migrations,
        Err(error) => {
            for plugin_name in &migrating_plugins {
                let _ = runtime_host
                    .record_failure(plugin_name, error.to_string())
                    .await;
            }
            return Err(error).context("failed to build builtin migration catalog");
        }
    };
    for (entry, plugin) in &resolved_lua_plugins {
        if !entry.enabled || !plugin.is_approved() {
            continue;
        }
        let loaded = load_lua_migrations(entry, &plugin.effective_permissions().database)
            .with_context(|| format!("failed to load migrations for runtime entry {}", entry.id));
        match loaded {
            Ok(loaded) => migrations.extend(loaded),
            Err(error) => {
                let _ = runtime_host
                    .record_failure(plugin.name(), error.to_string())
                    .await;
                return Err(error);
            }
        }
    }

    let db_path = {
        let guard = config.get().await;
        guard.database.path.clone()
    };

    if let Some(parent) = Path::new(&db_path).parent() {
        tokio::fs::create_dir_all(parent).await.ok();
    }

    let storage = match SqliteStorage::new(&db_path).await {
        Ok(storage) => storage,
        Err(error) => {
            for plugin_name in &migrating_plugins {
                let _ = runtime_host
                    .record_failure(plugin_name, error.to_string())
                    .await;
            }
            return Err(error).context("failed to open database");
        }
    };
    apply_runtime_migrations(&storage, &migrations, &runtime_host, &migrating_plugins).await?;

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

    let templates = TemplateService::new(&templates_dir)
        .with_context(|| format!("failed to init template root {}", templates_dir.display()))?;

    let ctx = SushiContext::new_with_runtime_profile_and_host(
        config,
        storage,
        jwt,
        templates,
        runtime_profile.clone(),
        runtime_host,
    );
    tracing_bridge::register_log_service(ctx.logs.clone());

    for entry in runtime_profile
        .entries()
        .iter()
        .filter(|entry| entry.enabled)
    {
        let Some(key) = entry.source.builtin_key() else {
            continue;
        };
        builtin_factories
            .activate(key, &ctx, entry)
            .await
            .with_context(|| format!("failed to activate runtime entry {}", entry.id))?;
    }

    // Load plugins
    for (entry, plugin) in resolved_lua_plugins {
        let plugin_name = plugin.name().to_string();
        let plugin_path_id = plugin.path_id().to_string();
        let plugin_kind = plugin.kind();
        ctx.plugins
            .register_profile_plugin_manifest(
                plugin.manifest(),
                plugin.effective_permissions(),
                &plugin_path_id,
                plugin_kind,
                entry.enabled,
                entry.required,
            )
            .await;
        if !entry.enabled {
            tracing::info!(
                plugin = plugin_name,
                entry = %entry.id,
                "plugin is disabled by runtime profile; skipping init"
            );
            ctx.plugins.mark_plugin_loaded(&plugin_name, false).await;
            let _ = ctx.runtime_host.mark_inactive(&plugin_name).await;
            continue;
        }
        if !plugin.is_approved() {
            let message = format!(
                "plugin '{plugin_name}' is not approved by runtime entry '{}'; set grants.approved = true and restart",
                entry.id
            );
            if entry.required {
                anyhow::bail!(
                    "required runtime entry {} failed approval: {message}",
                    entry.id
                );
            }
            tracing::warn!("{message}; skipping init");
            ctx.logs.warn(&format!("{message}; skipping init")).await;
            ctx.plugins.mark_plugin_loaded(&plugin_name, false).await;
            let _ = ctx
                .runtime_host
                .record_failure(&plugin_name, message.clone())
                .await;
            continue;
        }
        let enabled_before_init = match ctx.plugins.plugin_runtime_enabled(&plugin_name).await {
            Ok(enabled) => enabled,
            Err(err) => {
                let message = format!(
                    "failed to resolve runtime state before init for plugin {plugin_name}: {err}; skipping init"
                );
                tracing::warn!("{message}");
                ctx.logs.warn(&message).await;
                ctx.plugins.mark_plugin_loaded(&plugin_name, false).await;
                let _ = ctx
                    .runtime_host
                    .record_failure(&plugin_name, message.clone())
                    .await;
                continue;
            }
        };
        if !enabled_before_init && !entry.required {
            tracing::info!("plugin {plugin_name} is disabled by governance state; skipping init");
            ctx.plugins.mark_plugin_loaded(&plugin_name, false).await;
            let _ = ctx.runtime_host.mark_inactive(&plugin_name).await;
            continue;
        }

        if let Err(e) = ctx.runtime_host.activate(&ctx, &plugin_name).await {
            if entry.required {
                return Err(anyhow::Error::new(e)).with_context(|| {
                    format!("required runtime entry {} failed to activate", entry.id)
                });
            }
            tracing::warn!("failed to init plugin {plugin_name}: {e}");
            ctx.logs
                .warn(&format!("failed to init plugin {plugin_name}: {e}"))
                .await;
            ctx.plugins.mark_plugin_loaded(&plugin_name, false).await;
            continue;
        }

        tracing::debug!("activated Lua plugin {plugin_name}");
    }

    sushi_core::builtin::refresh_policy(&ctx).await?;

    Ok(ctx)
}

async fn apply_runtime_migrations(
    storage: &SqliteStorage,
    migrations: &[PluginMigration],
    runtime_host: &RuntimeHost,
    migrating_plugins: &[String],
) -> Result<()> {
    if let Err(error) = MigrationRunner::new(storage).apply(migrations).await {
        for plugin_name in migrating_plugins {
            let _ = runtime_host
                .record_failure(plugin_name, error.to_string())
                .await;
        }
        return Err(anyhow::Error::new(error)).context("failed to apply runtime migration catalog");
    }
    for plugin_name in migrating_plugins {
        runtime_host
            .complete_migration(plugin_name)
            .await
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    }
    Ok(())
}

pub(crate) fn builtin_factories() -> Result<BuiltinFactoryRegistry> {
    builtin_factories_with_role("admin", None)
}

fn builtin_factories_with_role(
    role: &str,
    config_path: Option<&Path>,
) -> Result<BuiltinFactoryRegistry> {
    let mut factories = BuiltinFactoryRegistry::new();
    factories.register(sushi_core::builtin::HostCoreFactory)?;
    factories.register(crate::builtin::HostCliFactory::new(
        role,
        config_path.unwrap_or_else(|| Path::new("config.toml")),
    ))?;
    factories.register(sushi_core::builtin::PolicyFactory)?;
    factories.register(sushi_api::builtin::IdentityFactory)?;
    factories.register(sushi_api::builtin::ApiCoreFactory)?;
    factories.register(sushi_admin::builtin::AdminShellFactory)?;
    factories.register(sushi_admin::builtin::HostAdminFactory)?;
    factories.register(sushi_admin::builtin::GovernanceFactory)?;
    factories.register(sushi_admin::builtin::RbacAdminFactory)?;
    factories.register(sushi_admin::builtin::MenuAdminFactory)?;
    Ok(factories)
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{header, Request, StatusCode};
    use std::time::{SystemTime, UNIX_EPOCH};
    use sushi_core::runtime::PluginLifecycleState;
    use sushi_core::storage::Storage;
    use tower::ServiceExt;

    fn unique_temp_root(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("sushi-{label}-{unique}"))
    }

    fn write_test_plugin(root: &Path, name: &str, init_lua: &str) {
        let plugin_dir = root.join("plugins").join("third_party").join(name);
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(
            plugin_dir.join("plugin.toml"),
            format!(
                r#"
schema_version = 1

[plugin]
name = "{name}"
version = "0.1.0"
entry = "init.lua"

[permissions]
routes = true
"#
            ),
        )
        .unwrap();
        std::fs::write(plugin_dir.join("init.lua"), init_lua).unwrap();
    }

    fn write_official_profile_stub(
        root: &Path,
        path_name: &str,
        plugin_name: &str,
        init_lua: &str,
    ) {
        let plugin_dir = root.join("plugins").join("official").join(path_name);
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(
            plugin_dir.join("plugin.toml"),
            format!(
                r#"
schema_version = 1

[plugin]
name = "{plugin_name}"
version = "0.1.0"
entry = "init.lua"

[permissions]
routes = true
commands = true
admin = true
database = "admin"

[policies]
scopes = ["api.profile.*", "admin.profile.*", "cli.profile.*"]
"#
            ),
        )
        .unwrap();
        std::fs::write(plugin_dir.join("init.lua"), init_lua).unwrap();
        let migration = match path_name {
            "cms" => Some((
                "007_cms.sql",
                include_str!("../../../migrations/007_cms.sql"),
            )),
            "kv-store" => Some((
                "002_kv_store.sql",
                include_str!("../../../migrations/002_kv_store.sql"),
            )),
            _ => None,
        };
        if let Some((file_name, sql)) = migration {
            std::fs::create_dir_all(plugin_dir.join("migrations")).unwrap();
            std::fs::write(plugin_dir.join("migrations").join(file_name), sql).unwrap();
        }
    }

    fn write_profile_config(root: &Path, database_path: &Path) -> PathBuf {
        let config_path = root.join("config.toml");
        std::fs::write(
            &config_path,
            format!(
                r#"
[database]
path = "{}"

[plugins]
directory = "plugins"

[web]
templates_dir = "templates"
static_dir = "static"
static_url_prefix = "/static"

[runtime]
profile = "default"
profiles_dir = "profiles"
bundles_dir = "bundles"
"#,
                database_path.display()
            ),
        )
        .unwrap();
        config_path
    }

    #[tokio::test]
    async fn failed_runtime_migration_sets_plugin_status_to_failed() {
        let temp_root = unique_temp_root("migration-status-failure");
        write_test_plugin(
            &temp_root,
            "migration-status",
            "sushi.init = function() end",
        );
        let plugin_dir = temp_root
            .join("plugins")
            .join("third_party")
            .join("migration-status");
        let plugin = LuaPlugin::load_dir(&plugin_dir, "third_party/migration-status")
            .await
            .unwrap();
        let runtime_host = RuntimeHost::new();
        runtime_host.register_lua_source(&plugin, false).await;
        runtime_host.begin_migration(plugin.name()).await.unwrap();

        let storage = SqliteStorage::new_in_memory().await.unwrap();
        let migration = PluginMigration::new(
            "third_party/migration-status",
            "001_invalid",
            "THIS IS NOT VALID SQL",
        )
        .unwrap();
        let error = apply_runtime_migrations(
            &storage,
            &[migration],
            &runtime_host,
            &[plugin.name().to_string()],
        )
        .await
        .unwrap_err();

        assert!(error
            .to_string()
            .contains("failed to apply runtime migration catalog"));
        let status = runtime_host.status(plugin.name()).await.unwrap();
        assert_eq!(status.state, PluginLifecycleState::Failed);
        assert!(status.last_error.is_some());

        std::fs::remove_dir_all(temp_root).ok();
    }

    #[tokio::test]
    async fn bootstrap_skips_disabled_plugin_init_side_effects() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temp_root = std::env::temp_dir().join(format!("sushi-bootstrap-skip-{unique}"));
        std::fs::create_dir_all(&temp_root).unwrap();

        let data_dir = temp_root.join("data");
        std::fs::create_dir_all(&data_dir).unwrap();
        let db_path = data_dir.join("sushi.db");

        let plugins_dir = temp_root.join("plugins");
        let plugin_dir = plugins_dir.join("third_party").join("skip-probe");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(
            plugin_dir.join("plugin.toml"),
            r#"
schema_version = 1

[plugin]
name = "skip-probe"
version = "0.1.0"
entry = "init.lua"

[permissions]
routes = true
"#,
        )
        .unwrap();
        std::fs::write(
            plugin_dir.join("init.lua"),
            r#"
sushi.api.route("GET", "/api/bootstrap-skip-proof", function()
    return "init-ran"
end)
"#,
        )
        .unwrap();

        let profiles_dir = temp_root.join("profiles");
        let bundles_dir = temp_root.join("bundles");
        std::fs::create_dir_all(&profiles_dir).unwrap();
        std::fs::create_dir_all(&bundles_dir).unwrap();
        std::fs::write(
            profiles_dir.join("default.toml"),
            "schema_version = 1\nname = \"default\"\nbundles = [\"test\"]\n",
        )
        .unwrap();
        std::fs::write(
            bundles_dir.join("test.toml"),
            r#"
schema_version = 1
name = "test"

[[entries]]
id = "host.core"
source = "builtin:host-core"
enabled = true
required = true

[[entries]]
id = "policy.core"
source = "builtin:policy"
enabled = true
required = true

[[entries]]
id = "skip-probe.default"
source = "lua:third_party/skip-probe"
enabled = true
required = false

[entries.grants]
approved = true
"#,
        )
        .unwrap();

        let db_path_string = db_path.to_string_lossy().to_string();
        let storage = SqliteStorage::new(&db_path_string).await.unwrap();
        let mut migrations = sushi_core::runtime::historical_host_core_migrations().unwrap();
        migrations.extend(sushi_core::runtime::historical_policy_migrations().unwrap());
        MigrationRunner::new(&storage)
            .apply(&migrations)
            .await
            .unwrap();
        storage
            .execute(
                "INSERT INTO plugin_state (name, plugin_id, source_kind, enabled, loaded, version, updated_by, reason, updated_at) VALUES ('skip-probe', 'third_party/skip-probe', 'third_party', 0, 0, '0.1.0', 'seed', 'disabled for bootstrap test', datetime('now'))",
                vec![],
            )
            .await
            .unwrap();
        drop(storage);

        let config_path = temp_root.join("config.toml");
        let templates_dir = temp_root.join("templates");
        let static_dir = temp_root.join("static");
        let config_toml = format!(
            r#"
[database]
path = "{}"

[plugins]
directory = "{}"

[runtime]
profiles_dir = "{}"
bundles_dir = "{}"

[web]
templates_dir = "{}"
static_dir = "{}"
static_url_prefix = "/static"
"#,
            db_path.display(),
            plugins_dir.display(),
            profiles_dir.display(),
            bundles_dir.display(),
            templates_dir.display(),
            static_dir.display()
        );
        std::fs::write(&config_path, config_toml).unwrap();

        let ctx = bootstrap(Some(&config_path)).await.unwrap();

        assert!(
            ctx.plugins
                .call_api_handler("GET", "/api/bootstrap-skip-proof", None)
                .await
                .is_none(),
            "disabled plugin side effects should not be registered during bootstrap"
        );

        let enabled = ctx
            .set_plugin_enabled(
                "skip-probe",
                true,
                Some("admin"),
                Some("enable after bootstrap"),
            )
            .await
            .unwrap();
        assert!(enabled.enabled);
        assert!(enabled.loaded);
        assert_eq!(
            ctx.plugins
                .call_api_handler("GET", "/api/bootstrap-skip-proof", None)
                .await
                .unwrap()
                .unwrap(),
            "init-ran"
        );

        let disabled = ctx
            .set_plugin_enabled(
                "skip-probe",
                false,
                Some("admin"),
                Some("disable after enable"),
            )
            .await
            .unwrap();
        assert!(!disabled.enabled);
        assert!(!disabled.loaded);
        assert!(ctx
            .plugins
            .call_api_handler("GET", "/api/bootstrap-skip-proof", None)
            .await
            .is_none());

        std::fs::remove_dir_all(&temp_root).ok();
    }

    #[tokio::test]
    async fn profile_entries_control_owner_required_and_disabled_activation() {
        let temp_root = unique_temp_root("profile-bootstrap");
        std::fs::create_dir_all(temp_root.join("profiles")).unwrap();
        std::fs::create_dir_all(temp_root.join("bundles")).unwrap();
        write_test_plugin(
            &temp_root,
            "required-probe",
            r#"
sushi.api.route("GET", "/api/profile-owner", function()
    return "active"
end)
"#,
        );
        write_test_plugin(
            &temp_root,
            "disabled-probe",
            r#"
sushi.api.route("GET", "/api/profile-disabled", function()
    return "unexpected"
end)
"#,
        );
        std::fs::write(
            temp_root.join("bundles/test.toml"),
            r#"
schema_version = 1
name = "test"

[[entries]]
id = "host.core"
source = "builtin:host-core"
enabled = true
required = true

[[entries]]
id = "policy.core"
source = "builtin:policy"
enabled = true
required = true

[[entries]]
id = "probe.required"
source = "lua:third_party/required-probe"
enabled = true
required = true

[entries.grants]
approved = true

[[entries]]
id = "probe.disabled"
source = "lua:third_party/disabled-probe"
enabled = false
required = false

[entries.grants]
approved = true
"#,
        )
        .unwrap();
        std::fs::write(
            temp_root.join("profiles/default.toml"),
            "schema_version = 1\nname = \"default\"\nbundles = [\"test\"]\n",
        )
        .unwrap();
        let config_path = write_profile_config(&temp_root, &temp_root.join("data/sushi.db"));

        let ctx = bootstrap(Some(&config_path)).await.unwrap();
        let inspection = ctx.plugins.capability_snapshot().await.inspect();
        let route = inspection
            .iter()
            .find(|entry| entry.key == "http:api:GET:/api/profile-owner")
            .expect("required profile route should be active");
        assert_eq!(route.owner.as_str(), "probe.required");
        assert!(ctx.runtime_host.is_required("required-probe").await);
        assert!(ctx
            .plugins
            .call_api_handler("GET", "/api/profile-disabled", None)
            .await
            .is_none());
        let states = ctx.plugins.list_plugins().await;
        let disabled = states
            .iter()
            .find(|plugin| plugin.name == "disabled-probe")
            .unwrap();
        assert!(!disabled.enabled);
        assert!(!disabled.loaded);
        let error = ctx
            .set_plugin_enabled("required-probe", false, Some("admin"), Some("must fail"))
            .await
            .unwrap_err();
        assert!(error.starts_with("required_plugin_toggle_forbidden:"));

        std::fs::remove_dir_all(&temp_root).ok();
    }

    #[tokio::test]
    async fn unapproved_optional_plugin_never_executes_or_publishes_effects() {
        let temp_root = unique_temp_root("unapproved-optional");
        std::fs::create_dir_all(temp_root.join("profiles")).unwrap();
        std::fs::create_dir_all(temp_root.join("bundles")).unwrap();
        write_test_plugin(
            &temp_root,
            "unapproved-probe",
            "error('unapproved plugin code executed')",
        );
        std::fs::write(
            temp_root.join("bundles/test.toml"),
            r#"
schema_version = 1
name = "test"

[[entries]]
id = "host.core"
source = "builtin:host-core"
enabled = true
required = true

[[entries]]
id = "policy.core"
source = "builtin:policy"
enabled = true
required = true

[[entries]]
id = "probe.unapproved"
source = "lua:third_party/unapproved-probe"
enabled = true
required = false
"#,
        )
        .unwrap();
        std::fs::write(
            temp_root.join("profiles/default.toml"),
            "schema_version = 1\nname = \"default\"\nbundles = [\"test\"]\n",
        )
        .unwrap();
        let config_path = write_profile_config(&temp_root, &temp_root.join("data/sushi.db"));

        let ctx = bootstrap(Some(&config_path)).await.unwrap();
        let plugin = ctx
            .plugins
            .list_plugins()
            .await
            .into_iter()
            .find(|plugin| plugin.name == "unapproved-probe")
            .unwrap();
        assert!(plugin.enabled);
        assert!(!plugin.loaded);
        assert!(ctx
            .plugins
            .capability_snapshot()
            .await
            .registration_ids_for_owner(
                &sushi_core::runtime::PluginInstanceId::new("probe.unapproved").unwrap()
            )
            .is_empty());
        assert!(ctx.runtime_host.handle("unapproved-probe").await.is_none());

        std::fs::remove_dir_all(&temp_root).ok();
    }

    #[tokio::test]
    async fn unapproved_optional_plugin_migrations_are_excluded_from_bootstrap() {
        let temp_root = unique_temp_root("unapproved-optional-migration");
        std::fs::create_dir_all(temp_root.join("profiles")).unwrap();
        std::fs::create_dir_all(temp_root.join("bundles")).unwrap();
        write_official_profile_stub(
            &temp_root,
            "migration-probe",
            "migration-probe",
            "error('unapproved plugin code executed')",
        );
        let migrations_dir = temp_root.join("plugins/official/migration-probe/migrations");
        std::fs::create_dir_all(&migrations_dir).unwrap();
        std::fs::write(
            migrations_dir.join("001_probe.sql"),
            "CREATE TABLE unapproved_optional_migration_effect (id INTEGER PRIMARY KEY);",
        )
        .unwrap();
        std::fs::write(
            temp_root.join("bundles/test.toml"),
            r#"
schema_version = 1
name = "test"

[[entries]]
id = "host.core"
source = "builtin:host-core"
enabled = true
required = true

[[entries]]
id = "policy.core"
source = "builtin:policy"
enabled = true
required = true

[[entries]]
id = "probe.unapproved"
source = "lua:official/migration-probe"
enabled = true
required = false
"#,
        )
        .unwrap();
        std::fs::write(
            temp_root.join("profiles/default.toml"),
            "schema_version = 1\nname = \"default\"\nbundles = [\"test\"]\n",
        )
        .unwrap();
        let database_path = temp_root.join("data/sushi.db");
        let config_path = write_profile_config(&temp_root, &database_path);

        let ctx = bootstrap(Some(&config_path)).await.unwrap();
        let rows = ctx
            .db
            .query(
                "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'unapproved_optional_migration_effect'",
                Vec::new(),
            )
            .await
            .unwrap();
        assert!(rows.is_empty());
        ctx.shutdown().await;

        std::fs::remove_dir_all(&temp_root).ok();
    }

    #[tokio::test]
    async fn unapproved_required_plugin_aborts_with_recovery_hint() {
        let temp_root = unique_temp_root("unapproved-required");
        std::fs::create_dir_all(temp_root.join("profiles")).unwrap();
        std::fs::create_dir_all(temp_root.join("bundles")).unwrap();
        write_test_plugin(
            &temp_root,
            "unapproved-required",
            "error('must not execute')",
        );
        std::fs::write(
            temp_root.join("bundles/test.toml"),
            r#"
schema_version = 1
name = "test"

[[entries]]
id = "host.core"
source = "builtin:host-core"
enabled = true
required = true

[[entries]]
id = "probe.unapproved"
source = "lua:third_party/unapproved-required"
enabled = true
required = true
"#,
        )
        .unwrap();
        std::fs::write(
            temp_root.join("profiles/default.toml"),
            "schema_version = 1\nname = \"default\"\nbundles = [\"test\"]\n",
        )
        .unwrap();
        let config_path = write_profile_config(&temp_root, &temp_root.join("data/sushi.db"));

        let error = match bootstrap(Some(&config_path)).await {
            Ok(_) => panic!("required unapproved plugin must abort bootstrap"),
            Err(error) => error.to_string(),
        };
        assert!(error.contains("required runtime entry probe.unapproved source 'lua:third_party/unapproved-required' is not approved"));
        assert!(error.contains("set grants.approved = true and restart"));
        assert!(!temp_root.join("data/sushi.db").exists());

        std::fs::remove_dir_all(&temp_root).ok();
    }

    #[tokio::test]
    async fn invalid_profile_fails_before_database_creation() {
        let temp_root = unique_temp_root("profile-fail-closed");
        std::fs::create_dir_all(temp_root.join("profiles")).unwrap();
        std::fs::write(
            temp_root.join("profiles/default.toml"),
            "schema_version = 1\nname = \"default\"\nbundles = [\"missing\"]\n",
        )
        .unwrap();
        let database_path = temp_root.join("data/sushi.db");
        let config_path = write_profile_config(&temp_root, &database_path);

        let error = match bootstrap(Some(&config_path)).await {
            Ok(_) => panic!("invalid profile should fail before bootstrap"),
            Err(error) => error,
        };
        assert!(error
            .to_string()
            .contains("failed to resolve runtime profile"));
        assert!(!database_path.exists());

        std::fs::remove_dir_all(&temp_root).ok();
    }

    #[tokio::test]
    async fn required_profile_activation_failure_aborts_bootstrap() {
        let temp_root = unique_temp_root("required-profile-failure");
        std::fs::create_dir_all(temp_root.join("profiles")).unwrap();
        std::fs::create_dir_all(temp_root.join("bundles")).unwrap();
        write_test_plugin(&temp_root, "broken-required", "error('activation failed')");
        std::fs::write(
            temp_root.join("bundles/test.toml"),
            r#"
schema_version = 1
name = "test"

[[entries]]
id = "host.core"
source = "builtin:host-core"
enabled = true
required = true

[[entries]]
id = "policy.core"
source = "builtin:policy"
enabled = true
required = true

[[entries]]
id = "probe.broken"
source = "lua:third_party/broken-required"
enabled = true
required = true

[entries.grants]
approved = true
"#,
        )
        .unwrap();
        std::fs::write(
            temp_root.join("profiles/default.toml"),
            "schema_version = 1\nname = \"default\"\nbundles = [\"test\"]\n",
        )
        .unwrap();
        let config_path = write_profile_config(&temp_root, &temp_root.join("data/sushi.db"));

        let error = match bootstrap(Some(&config_path)).await {
            Ok(_) => panic!("required activation failure should abort bootstrap"),
            Err(error) => error,
        };
        assert!(error
            .to_string()
            .contains("required runtime entry probe.broken failed to activate"));

        std::fs::remove_dir_all(&temp_root).ok();
    }

    #[tokio::test]
    async fn shipped_profiles_produce_golden_capability_maps() {
        let temp_root = unique_temp_root("profile-capability-golden");
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        write_official_profile_stub(
            &temp_root,
            "cms",
            "cms",
            r#"
function app.init()
    app.api.route("GET", "/api/profile/cms", function() return "cms" end)
    app.cli.command("cms-probe", "CMS probe", function() return "cms" end)
    app.admin.page("/admin/profile/cms", "CMS probe", function() return "cms" end)
end
"#,
        );
        write_official_profile_stub(
            &temp_root,
            "file-browser",
            "file-browser",
            r#"
function app.init()
    app.api.route("GET", "/app/profile/files", function() return "files" end)
end
"#,
        );
        write_official_profile_stub(
            &temp_root,
            "kv-store",
            "kv-store",
            r#"
function app.init()
    app.api.route("GET", "/api/profile/kv", function() return "kv" end)
    app.cli.command("kv-probe", "KV probe", function() return "kv" end, { policy = "cli.profile.kv" })
    app.admin.page("/admin/profile/kv", "KV probe", function() return "kv" end)
end
"#,
        );
        let expected_full = include_str!("../tests/fixtures/profile/full_capabilities.txt")
            .lines()
            .map(str::to_string)
            .collect::<Vec<_>>();

        for (profile, expected_api, expected_admin) in [
            ("default", true, true),
            ("api", true, false),
            ("admin", false, true),
            ("minimal", false, false),
        ] {
            let profile_root = temp_root.join(profile);
            let database_path = profile_root.join("data/sushi.db");
            std::fs::create_dir_all(&profile_root).unwrap();
            std::fs::create_dir_all(profile_root.join("httpshared")).unwrap();
            std::fs::create_dir_all(profile_root.join("temp")).unwrap();
            let config_path = profile_root.join("config.toml");
            std::fs::write(
                &config_path,
                format!(
                    r#"
[database]
path = "{}"

[plugins]
directory = "{}"

[web]
templates_dir = "templates"
static_dir = "static"
static_url_prefix = "/static"

[runtime]
profile = "{profile}"
profiles_dir = "{}"
bundles_dir = "{}"
"#,
                    database_path.display(),
                    temp_root.join("plugins").display(),
                    repo_root.join("profiles").display(),
                    repo_root.join("bundles").display(),
                ),
            )
            .unwrap();

            let ctx = bootstrap(Some(&config_path)).await.unwrap();
            let policy = ctx
                .plugins
                .list_plugins()
                .await
                .into_iter()
                .find(|plugin| plugin.name == "policy")
                .expect("policy builtin should be registered");
            assert_eq!(policy.plugin_id, "builtin/policy", "profile {profile}");
            assert_eq!(policy.source_kind, "builtin", "profile {profile}");
            assert!(policy.enabled, "profile {profile}");
            assert!(policy.loaded, "profile {profile}");
            assert_eq!(policy.permissions.database, "admin", "profile {profile}");
            assert!(
                ctx.plugins.is_plugin_required("policy").await,
                "profile {profile}"
            );
            let toggle_error = ctx
                .set_plugin_enabled("policy", false, Some("test"), Some("required guard"))
                .await
                .unwrap_err();
            assert_eq!(
                toggle_error,
                "required_plugin_toggle_forbidden: plugin 'policy' must be changed through profile and restart",
                "profile {profile}"
            );
            assert_eq!(
                ctx.authorizer.has_command_binding("cli", "kv-probe").await,
                profile != "minimal",
                "profile {profile} Lua policy binding hydration"
            );
            let migration_rows = ctx
                .db
                .query(
                    "SELECT plugin_id, migration_id FROM plugin_migrations ORDER BY migration_id",
                    vec![],
                )
                .await
                .unwrap();
            let migration_ids = migration_rows
                .iter()
                .filter_map(|row| row.get("migration_id").and_then(|value| value.as_str()))
                .collect::<Vec<_>>();
            assert!(migration_ids.contains(&"001_init"));
            assert!(migration_ids.contains(&"003_rbac"));
            assert!(migration_ids.contains(&"006_unified_policy_v2"));
            assert!(migration_ids.contains(&"008_plugin_governance_v1"));
            let policy_migration_owners = migration_rows
                .iter()
                .filter(|row| {
                    matches!(
                        row.get("migration_id").and_then(|value| value.as_str()),
                        Some("003_rbac" | "006_unified_policy_v2")
                    )
                })
                .filter_map(|row| row.get("plugin_id").and_then(|value| value.as_str()))
                .collect::<Vec<_>>();
            assert_eq!(
                policy_migration_owners,
                vec!["builtin/policy", "builtin/policy"],
                "profile {profile}"
            );
            if expected_admin {
                assert!(migration_ids.contains(&"004_menu"));
                assert!(migration_ids.contains(&"005_menus_rbac"));
                assert!(migration_ids.contains(&"009_menu_contributions"));
            } else {
                assert!(!migration_ids.contains(&"004_menu"));
                assert!(!migration_ids.contains(&"005_menus_rbac"));
                assert!(!migration_ids.contains(&"009_menu_contributions"));
            }
            if profile != "minimal" {
                assert!(migration_ids.contains(&"007_cms"));
                assert!(migration_ids.contains(&"002_kv_store"));
            } else {
                assert!(!migration_ids.contains(&"007_cms"));
                assert!(!migration_ids.contains(&"002_kv_store"));
            }
            let actual = ctx
                .plugins
                .capability_snapshot()
                .await
                .inspect()
                .into_iter()
                .map(|entry| format!("{}\towner={}", entry.key, entry.owner))
                .collect::<Vec<_>>();
            let expected = expected_full
                .iter()
                .filter(|entry| expected_api || !entry.ends_with("owner=identity.core"))
                .filter(|entry| expected_api || !entry.ends_with("owner=api.core"))
                .filter(|entry| expected_admin || !entry.ends_with("owner=admin.shell"))
                .filter(|entry| expected_admin || !entry.ends_with("owner=host.admin"))
                .filter(|entry| expected_admin || !entry.ends_with("owner=governance.admin"))
                .filter(|entry| expected_admin || !entry.ends_with("owner=rbac.admin"))
                .filter(|entry| expected_admin || !entry.ends_with("owner=menu.admin"))
                .filter(|entry| profile != "minimal" || !entry.ends_with("owner=cms.default"))
                .filter(|entry| profile != "minimal" || !entry.ends_with("owner=kv-store.default"))
                .filter(|entry| {
                    profile != "minimal" || !entry.ends_with("owner=file-browser.default")
                })
                .cloned()
                .collect::<Vec<_>>();
            assert_eq!(actual, expected, "profile {profile}");
            assert_eq!(
                ctx.plugins
                    .capability_snapshot()
                    .await
                    .has_transport(sushi_core::runtime::HttpSurface::Api),
                expected_api,
                "profile {profile} api surface"
            );
            assert_eq!(
                ctx.plugins
                    .capability_snapshot()
                    .await
                    .has_transport(sushi_core::runtime::HttpSurface::Admin),
                expected_admin,
                "profile {profile} Admin Shell surface"
            );
            assert_eq!(
                ctx.runtime_profile.has_enabled_builtin("governance"),
                expected_admin,
                "profile {profile} governance surface"
            );
            assert_eq!(
                ctx.runtime_profile.has_enabled_builtin("host-admin"),
                expected_admin,
                "profile {profile} admin surface"
            );
            assert_eq!(
                ctx.runtime_profile.has_enabled_builtin("rbac-admin"),
                expected_admin,
                "profile {profile} RBAC Admin surface"
            );
            assert_eq!(
                ctx.runtime_profile.has_enabled_builtin("menu-admin"),
                expected_admin,
                "profile {profile} Menu Admin surface"
            );
            assert!(
                ctx.runtime_profile.has_enabled_builtin("policy"),
                "profile {profile} policy surface"
            );
        }

        std::fs::remove_dir_all(&temp_root).ok();
    }

    #[tokio::test]
    async fn shipped_profile_http_smoke_matrix_and_optional_lifecycle() {
        let temp_root = unique_temp_root("profile-http-smoke");
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        write_official_profile_stub(
            &temp_root,
            "cms",
            "cms",
            r#"
function app.init()
    app.capability.register({ surface = "api", method = "GET", path = "/app/profile/cms", handler = function() return "cms" end, public = true })
end
"#,
        );
        write_official_profile_stub(
            &temp_root,
            "file-browser",
            "file-browser",
            r#"
function app.init()
    app.capability.register({ surface = "api", method = "GET", path = "/app/files", handler = function() return "files" end, public = true })
end
"#,
        );
        let file_browser_static = temp_root.join("plugins/official/file-browser/web/static");
        std::fs::create_dir_all(&file_browser_static).unwrap();
        std::fs::write(
            file_browser_static.join("file_browser.css"),
            ".file-browser { display: block; }",
        )
        .unwrap();
        write_official_profile_stub(
            &temp_root,
            "kv-store",
            "kv-store",
            r#"
function app.init()
    app.capability.register({ surface = "api", method = "GET", path = "/api/profile/kv", handler = function() return "kv" end, public = true })
end
"#,
        );

        for (profile, expected_api, expected_admin) in [
            ("default", true, true),
            ("api", true, false),
            ("admin", false, true),
            ("minimal", false, false),
        ] {
            let profile_root = temp_root.join(format!("http-{profile}"));
            std::fs::create_dir_all(profile_root.join("httpshared")).unwrap();
            std::fs::create_dir_all(profile_root.join("temp")).unwrap();
            let config_path = profile_root.join("config.toml");
            std::fs::write(
                &config_path,
                format!(
                    r#"
[database]
path = "{}"

[plugins]
directory = "{}"

[web]
templates_dir = "{}"
static_dir = "{}"
static_url_prefix = "/static"

[runtime]
profile = "{profile}"
profiles_dir = "{}"
bundles_dir = "{}"
"#,
                    profile_root.join("data/sushi.db").display(),
                    temp_root.join("plugins").display(),
                    repo_root.join("web/templates").display(),
                    repo_root.join("web/static").display(),
                    repo_root.join("profiles").display(),
                    repo_root.join("bundles").display(),
                ),
            )
            .unwrap();

            let ctx = bootstrap(Some(&config_path)).await.unwrap();
            let app = crate::commands::serve::build_router(&ctx).await;
            let status = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri("/health")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap()
                .status();
            assert_eq!(status, StatusCode::OK, "profile {profile} health");

            let status = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri("/api/users")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap()
                .status();
            assert_eq!(
                status,
                if expected_api {
                    StatusCode::UNAUTHORIZED
                } else {
                    StatusCode::NOT_FOUND
                },
                "profile {profile} API surface"
            );

            if expected_api {
                let viewer_token = ctx.jwt.create_access_token(2, "viewer", "viewer").unwrap();
                let status = app
                    .clone()
                    .oneshot(
                        Request::builder()
                            .uri("/api/users")
                            .header(header::AUTHORIZATION, format!("Bearer {viewer_token}"))
                            .body(Body::empty())
                            .unwrap(),
                    )
                    .await
                    .unwrap()
                    .status();
                assert_eq!(
                    status,
                    StatusCode::FORBIDDEN,
                    "profile {profile} API policy"
                );
            }

            let admin_token = ctx.jwt.create_access_token(1, "admin", "admin").unwrap();
            let admin_status = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri("/admin/")
                        .header(header::AUTHORIZATION, format!("Bearer {admin_token}"))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap()
                .status();
            assert_eq!(
                admin_status,
                if expected_admin {
                    StatusCode::OK
                } else {
                    StatusCode::NOT_FOUND
                },
                "profile {profile} Admin surface"
            );

            let missing_status = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri("/admin/not-a-capability")
                        .header(header::AUTHORIZATION, format!("Bearer {admin_token}"))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap()
                .status();
            assert_eq!(
                missing_status,
                StatusCode::NOT_FOUND,
                "profile {profile} 404"
            );

            if profile == "api" {
                for path in [
                    "/app/files",
                    "/static/plugins/official/file-browser/file_browser.css",
                    "/static/css/style.css",
                ] {
                    let status = app
                        .clone()
                        .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
                        .await
                        .unwrap()
                        .status();
                    assert_eq!(status, StatusCode::OK, "API profile asset {path}");
                }
            }

            if profile == "default" {
                let status = app
                    .clone()
                    .oneshot(
                        Request::builder()
                            .uri("/api/profile/kv")
                            .body(Body::empty())
                            .unwrap(),
                    )
                    .await
                    .unwrap()
                    .status();
                assert_eq!(status, StatusCode::OK);
                ctx.set_plugin_enabled("kv-store", false, Some("test"), Some("smoke disable"))
                    .await
                    .unwrap();
                let status = app
                    .clone()
                    .oneshot(
                        Request::builder()
                            .uri("/api/profile/kv")
                            .body(Body::empty())
                            .unwrap(),
                    )
                    .await
                    .unwrap()
                    .status();
                assert_eq!(status, StatusCode::NOT_FOUND);
                ctx.set_plugin_enabled("kv-store", true, Some("test"), Some("smoke enable"))
                    .await
                    .unwrap();
                let status = app
                    .oneshot(
                        Request::builder()
                            .uri("/api/profile/kv")
                            .body(Body::empty())
                            .unwrap(),
                    )
                    .await
                    .unwrap()
                    .status();
                assert_eq!(status, StatusCode::OK);
            }

            ctx.shutdown().await;
        }

        std::fs::remove_dir_all(&temp_root).ok();
    }
}
