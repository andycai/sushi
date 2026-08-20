use crate::context::{PluginContext, SushiContext};
use crate::plugin::{DatabasePermission, Permissions};
use crate::runtime::{
    historical_host_core_migrations, historical_policy_migrations, BuiltinPluginFactory,
    MigrationError, PluginMigration, ResolvedRuntimeEntry, RuntimePluginSource,
};
use async_trait::async_trait;

pub struct HostCoreFactory;

#[async_trait]
impl BuiltinPluginFactory for HostCoreFactory {
    fn key(&self) -> &'static str {
        "host-core"
    }

    fn migrations(
        &self,
        _entry: &ResolvedRuntimeEntry,
    ) -> Result<Vec<PluginMigration>, MigrationError> {
        historical_host_core_migrations()
    }

    async fn activate(
        &self,
        ctx: &SushiContext,
        _plugin_ctx: &PluginContext,
        entry: &ResolvedRuntimeEntry,
    ) -> anyhow::Result<()> {
        validate_builtin_entry(entry, self.key())?;
        ctx.plugins
            .register_builtin_profile_plugin(
                "builtin/host-core",
                "host-core",
                env!("CARGO_PKG_VERSION"),
                "Trusted runtime host boundary",
                &Permissions::default(),
                entry.enabled,
                entry.required,
            )
            .await;
        ctx.plugins.mark_plugin_loaded("host-core", true).await;
        Ok(())
    }
}

pub struct PolicyFactory;

#[async_trait]
impl BuiltinPluginFactory for PolicyFactory {
    fn key(&self) -> &'static str {
        "policy"
    }

    fn migrations(
        &self,
        _entry: &ResolvedRuntimeEntry,
    ) -> Result<Vec<PluginMigration>, MigrationError> {
        historical_policy_migrations()
    }

    async fn activate(
        &self,
        ctx: &SushiContext,
        _plugin_ctx: &PluginContext,
        entry: &ResolvedRuntimeEntry,
    ) -> anyhow::Result<()> {
        activate_policy(ctx, entry).await
    }
}

pub async fn activate_policy(
    ctx: &SushiContext,
    entry: &ResolvedRuntimeEntry,
) -> anyhow::Result<()> {
    validate_builtin_entry(entry, "policy")?;

    let permissions = Permissions {
        routes: false,
        commands: false,
        admin: false,
        database: DatabasePermission::Admin,
    };
    ctx.plugins
        .register_builtin_profile_plugin(
            "builtin/policy",
            "policy",
            env!("CARGO_PKG_VERSION"),
            "Built-in policy authorizer",
            &permissions,
            entry.enabled,
            entry.required,
        )
        .await;
    ctx.plugins.mark_plugin_loaded("policy", false).await;
    Ok(())
}

pub async fn refresh_policy(ctx: &SushiContext) -> anyhow::Result<()> {
    ctx.refresh_authorizer_snapshot()
        .await
        .map_err(anyhow::Error::msg)
        .map_err(|error| error.context("failed to compile policy snapshot from database"))?;

    if ctx
        .plugins
        .list_plugins()
        .await
        .iter()
        .any(|plugin| plugin.name == "policy")
    {
        ctx.plugins.mark_plugin_loaded("policy", true).await;
    }
    Ok(())
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
