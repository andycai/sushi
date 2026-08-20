use crate::routes::{auth, users};
use async_trait::async_trait;
use std::sync::Arc;
use sushi_core::context::{PluginContext, SushiContext};
use sushi_core::plugin::{DatabasePermission, Permissions};
use sushi_core::runtime::BuiltinPluginFactory;
use sushi_core::runtime::{HttpSurface, ResolvedRuntimeEntry, RuntimePluginSource, TransportSpec};
use sushi_core::storage::Storage;

pub struct IdentityFactory;

#[async_trait]
impl BuiltinPluginFactory for IdentityFactory {
    fn key(&self) -> &'static str {
        "identity"
    }

    async fn activate(
        &self,
        ctx: &SushiContext,
        _plugin_ctx: &PluginContext,
        entry: &ResolvedRuntimeEntry,
    ) -> anyhow::Result<()> {
        activate_identity(ctx, entry).await
    }
}

pub struct ApiCoreFactory;

#[async_trait]
impl BuiltinPluginFactory for ApiCoreFactory {
    fn key(&self) -> &'static str {
        "api-core"
    }

    async fn activate(
        &self,
        ctx: &SushiContext,
        _plugin_ctx: &PluginContext,
        entry: &ResolvedRuntimeEntry,
    ) -> anyhow::Result<()> {
        activate_api_core(ctx, entry).await
    }
}

pub async fn activate_identity(
    ctx: &SushiContext,
    entry: &ResolvedRuntimeEntry,
) -> anyhow::Result<()> {
    validate_builtin_entry(entry, "identity")?;

    let permissions = Permissions {
        routes: true,
        commands: false,
        admin: false,
        database: DatabasePermission::ReadOnly,
    };
    ctx.plugins
        .register_builtin_profile_plugin(
            "builtin/identity",
            "identity",
            env!("CARGO_PKG_VERSION"),
            "Built-in identity API routes",
            &permissions,
            entry.enabled,
            entry.required,
        )
        .await;

    let storage: Arc<dyn Storage> = ctx.db.clone();
    let mut staged = ctx.plugins.stage_builtin_activation(entry.id.clone());
    staged.register_transport(TransportSpec::new(HttpSurface::Api));
    auth::register_builtin_routes(&mut staged, "identity", storage, Arc::clone(&ctx.jwt));
    publish_builtin(ctx, "identity", staged).await
}

pub async fn activate_api_core(
    ctx: &SushiContext,
    entry: &ResolvedRuntimeEntry,
) -> anyhow::Result<()> {
    validate_builtin_entry(entry, "api-core")?;

    let permissions = Permissions {
        routes: true,
        commands: false,
        admin: false,
        database: DatabasePermission::Admin,
    };
    ctx.plugins
        .register_builtin_profile_plugin(
            "builtin/api-core",
            "api-core",
            env!("CARGO_PKG_VERSION"),
            "Built-in users API routes",
            &permissions,
            entry.enabled,
            entry.required,
        )
        .await;

    let storage: Arc<dyn Storage> = ctx.db.clone();
    let mut staged = ctx.plugins.stage_builtin_activation(entry.id.clone());
    users::register_builtin_routes(&mut staged, "api-core", storage);
    publish_builtin(ctx, "api-core", staged).await
}

fn validate_builtin_entry(entry: &ResolvedRuntimeEntry, expected: &str) -> anyhow::Result<()> {
    let RuntimePluginSource::Builtin { key, .. } = &entry.source else {
        anyhow::bail!("runtime entry '{}' is not a builtin source", entry.id);
    };
    if key != expected {
        anyhow::bail!(
            "runtime entry '{}' uses builtin '{}', expected '{}'",
            entry.id,
            key,
            expected
        );
    }
    Ok(())
}

async fn publish_builtin(
    ctx: &SushiContext,
    plugin_name: &str,
    staged: sushi_core::runtime::StagedRegistrar,
) -> anyhow::Result<()> {
    let pending = ctx
        .plugins
        .prepare_owner_activation(staged)
        .await
        .map_err(|error| anyhow::anyhow!(error))?;
    pending.publish().await;
    ctx.plugins.mark_plugin_loaded(plugin_name, true).await;
    Ok(())
}
