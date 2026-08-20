use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use sushi_core::lua::loader::LuaPlugin;
use sushi_core::runtime::{
    load_lua_migrations, MigrationRunner, MigrationVerificationStatus, RuntimePluginSource,
};
use sushi_core::storage::sqlite::SqliteStorage;

pub async fn run(config_path: &Path, profile_override: Option<&str>) -> Result<()> {
    run_with_overlays(config_path, profile_override, &[]).await
}

pub async fn run_with_overlays(
    config_path: &Path,
    profile_override: Option<&str>,
    overlay_paths: &[PathBuf],
) -> Result<()> {
    let (config, profile) = crate::app::resolve_runtime_profile_with_overlays(
        Some(config_path),
        profile_override,
        overlay_paths,
    )
    .await
    .context("failed to resolve runtime profile")?;
    let factories = crate::app::builtin_factories()?;
    let mut migrations = factories
        .migrations_for_entries(profile.entries())
        .context("failed to compile builtin migration catalog")?;
    let mut issues = Vec::new();

    println!("profile: {}", profile.name());
    for entry in profile.entries() {
        let source = match &entry.source {
            RuntimePluginSource::Builtin { reference, .. } => reference.clone(),
            RuntimePluginSource::Lua { path_id, path, .. } => {
                let mut plugin = match LuaPlugin::load_dir(path, path_id).await {
                    Ok(plugin) => plugin,
                    Err(error) => {
                        issues.push(format!(
                            "entry '{}' source '{}': {error}; repair: fix the plugin manifest/source path or remove the entry from the profile",
                            entry.id,
                            entry.source.reference()
                        ));
                        println!(
                            "{}\tenabled={}\trequired={}\tsource=lua:{path_id} ({})\tstatus=invalid",
                            entry.id,
                            entry.enabled,
                            entry.required,
                            path.display()
                        );
                        continue;
                    }
                };
                plugin.apply_profile_grants(&entry.grants);
                if entry.enabled && !plugin.is_approved() {
                    let message = format!(
                        "entry '{}' source '{}': administrator approval is missing; repair: set grants.approved = true in the owning bundle/profile and restart",
                        entry.id,
                        entry.source.reference()
                    );
                    if entry.required {
                        issues.push(message);
                    } else {
                        println!("warning: {message}");
                    }
                }
                if entry.enabled && plugin.is_approved() {
                    match load_lua_migrations(entry, &plugin.effective_permissions().database) {
                        Ok(plugin_migrations) => migrations.extend(plugin_migrations),
                        Err(error) => issues.push(format!(
                            "entry '{}' source '{}': migration catalog invalid: {error}; repair: restore the published migration files and grant approved database write/admin access",
                            entry.id,
                            entry.source.reference()
                        )),
                    }
                }
                format!("lua:{path_id} ({})", path.display())
            }
        };
        println!(
            "{}\tenabled={}\trequired={}\tsource={source}\tstatus=resolved",
            entry.id, entry.enabled, entry.required
        );
    }

    let database_path = {
        let guard = config.get().await;
        PathBuf::from(&guard.database.path)
    };
    if database_path.exists() {
        match SqliteStorage::new(&database_path.to_string_lossy()).await {
            Ok(storage) => match MigrationRunner::new(&storage).verify(&migrations).await {
                Ok(entries) => {
                    for entry in entries {
                        let status = match entry.status {
                            MigrationVerificationStatus::Applied => "applied",
                            MigrationVerificationStatus::Pending => "pending",
                            MigrationVerificationStatus::LegacyBridge => "legacy-bridge",
                            MigrationVerificationStatus::RecoveryRequired => {
                                issues.push(format!(
                                    "migration '{}:{}' is partially applied; repair: back up the database, then restart Sushi to let the forward-only recovery complete",
                                    entry.plugin_id, entry.migration_id
                                ));
                                "recovery-required"
                            }
                        };
                        println!(
                            "migration:{}:{}\tstatus={status}",
                            entry.plugin_id, entry.migration_id
                        );
                    }
                }
                Err(error) => issues.push(format!(
                    "database '{}': migration verification failed: {error}; repair: restore the published migration file or recover from a matching database backup",
                    database_path.display()
                )),
            },
            Err(error) => issues.push(format!(
                "database '{}': cannot open for inspection: {error}; repair: verify path, permissions, and SQLite integrity",
                database_path.display()
            )),
        }
    } else {
        println!(
            "database:{}\tstatus=absent\taction=will-be-created-on-successful-bootstrap",
            database_path.display()
        );
    }

    if issues.is_empty() {
        println!("doctor: ok");
        return Ok(());
    }

    for issue in &issues {
        eprintln!("doctor: error: {issue}");
    }
    anyhow::bail!("doctor found {} blocking issue(s)", issues.len())
}
