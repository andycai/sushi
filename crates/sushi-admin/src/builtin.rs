use crate::routes::{
    config, dashboard, login, logs, menu, permissions, plugins, roles, users, workspace,
};
use async_trait::async_trait;
use sushi_core::context::{PluginContext, SushiContext};
use sushi_core::plugin::{DatabasePermission, Permissions};
use sushi_core::runtime::BuiltinPluginFactory;
use sushi_core::runtime::{
    historical_menu_admin_migrations, HttpSurface, MigrationError, PluginMigration,
    ResolvedRuntimeEntry, RuntimePluginSource, TransportSpec,
};

macro_rules! builtin_factory {
    ($factory:ident, $key:literal, $activate:ident) => {
        pub struct $factory;

        #[async_trait]
        impl BuiltinPluginFactory for $factory {
            fn key(&self) -> &'static str {
                $key
            }

            async fn activate(
                &self,
                ctx: &SushiContext,
                _plugin_ctx: &PluginContext,
                entry: &ResolvedRuntimeEntry,
            ) -> anyhow::Result<()> {
                $activate(ctx, entry).await
            }
        }
    };
}

builtin_factory!(AdminShellFactory, "admin-shell", activate_admin_shell);
builtin_factory!(HostAdminFactory, "host-admin", activate_host_admin);
builtin_factory!(GovernanceFactory, "governance", activate_governance);
builtin_factory!(RbacAdminFactory, "rbac-admin", activate_rbac_admin);

pub struct MenuAdminFactory;

#[async_trait]
impl BuiltinPluginFactory for MenuAdminFactory {
    fn key(&self) -> &'static str {
        "menu-admin"
    }

    fn migrations(
        &self,
        _entry: &ResolvedRuntimeEntry,
    ) -> Result<Vec<PluginMigration>, MigrationError> {
        historical_menu_admin_migrations()
    }

    async fn activate(
        &self,
        ctx: &SushiContext,
        _plugin_ctx: &PluginContext,
        entry: &ResolvedRuntimeEntry,
    ) -> anyhow::Result<()> {
        activate_menu_admin(ctx, entry).await
    }
}

pub async fn activate_host_admin(
    ctx: &SushiContext,
    entry: &ResolvedRuntimeEntry,
) -> anyhow::Result<()> {
    let RuntimePluginSource::Builtin { key, .. } = &entry.source else {
        anyhow::bail!("runtime entry '{}' is not a builtin source", entry.id);
    };
    if key != "host-admin" {
        anyhow::bail!(
            "runtime entry '{}' uses builtin '{}', expected 'host-admin'",
            entry.id,
            key
        );
    }

    let permissions = Permissions {
        routes: true,
        commands: false,
        admin: true,
        database: DatabasePermission::None,
    };
    ctx.plugins
        .register_builtin_profile_plugin(
            "builtin/host-admin",
            "host-admin",
            env!("CARGO_PKG_VERSION"),
            "Built-in Admin routes",
            &permissions,
            entry.enabled,
            entry.required,
        )
        .await;

    let mut staged = ctx.plugins.stage_builtin_activation(entry.id.clone());
    menu::register_builtin_capabilities(&mut staged);
    logs::register_builtin_capabilities(&mut staged, ctx.clone());
    config::register_builtin_capabilities(&mut staged, ctx.clone());
    plugins::register_builtin_capabilities(&mut staged, ctx.clone());
    let pending = ctx
        .plugins
        .prepare_owner_activation(staged)
        .await
        .map_err(|error| anyhow::anyhow!(error))?;
    pending.publish().await;
    ctx.plugins.mark_plugin_loaded("host-admin", true).await;
    Ok(())
}

pub async fn activate_governance(
    ctx: &SushiContext,
    entry: &ResolvedRuntimeEntry,
) -> anyhow::Result<()> {
    let RuntimePluginSource::Builtin { key, .. } = &entry.source else {
        anyhow::bail!("runtime entry '{}' is not a builtin source", entry.id);
    };
    if key != "governance" {
        anyhow::bail!(
            "runtime entry '{}' uses builtin '{}', expected 'governance'",
            entry.id,
            key
        );
    }

    let permissions = Permissions {
        routes: true,
        commands: false,
        admin: true,
        database: DatabasePermission::Admin,
    };
    ctx.plugins
        .register_builtin_profile_plugin(
            "builtin/governance",
            "governance",
            env!("CARGO_PKG_VERSION"),
            "Built-in plugin governance",
            &permissions,
            entry.enabled,
            entry.required,
        )
        .await;

    let mut staged = ctx.plugins.stage_builtin_activation(entry.id.clone());
    plugins::register_governance_capabilities(&mut staged, ctx.clone());
    let pending = ctx
        .plugins
        .prepare_owner_activation(staged)
        .await
        .map_err(|error| anyhow::anyhow!(error))?;
    pending.publish().await;
    ctx.plugins.mark_plugin_loaded("governance", true).await;
    Ok(())
}

pub async fn activate_admin_shell(
    ctx: &SushiContext,
    entry: &ResolvedRuntimeEntry,
) -> anyhow::Result<()> {
    let RuntimePluginSource::Builtin { key, .. } = &entry.source else {
        anyhow::bail!("runtime entry '{}' is not a builtin source", entry.id);
    };
    if key != "admin-shell" {
        anyhow::bail!(
            "runtime entry '{}' uses builtin '{}', expected 'admin-shell'",
            entry.id,
            key
        );
    }

    let permissions = Permissions {
        routes: true,
        commands: false,
        admin: true,
        database: DatabasePermission::None,
    };
    ctx.plugins
        .register_builtin_profile_plugin(
            "builtin/admin-shell",
            "admin-shell",
            env!("CARGO_PKG_VERSION"),
            "Built-in Admin shell",
            &permissions,
            entry.enabled,
            entry.required,
        )
        .await;

    let mut staged = ctx.plugins.stage_builtin_activation(entry.id.clone());
    staged.register_transport(TransportSpec::new(HttpSurface::Admin));
    dashboard::register_builtin_capabilities(&mut staged, ctx.clone());
    login::register_builtin_capabilities(&mut staged, ctx.clone());
    workspace::register_builtin_capabilities(&mut staged, ctx.clone());
    let pending = ctx
        .plugins
        .prepare_owner_activation(staged)
        .await
        .map_err(|error| anyhow::anyhow!(error))?;
    pending.publish().await;
    ctx.plugins.mark_plugin_loaded("admin-shell", true).await;
    Ok(())
}

pub async fn activate_rbac_admin(
    ctx: &SushiContext,
    entry: &ResolvedRuntimeEntry,
) -> anyhow::Result<()> {
    let RuntimePluginSource::Builtin { key, .. } = &entry.source else {
        anyhow::bail!("runtime entry '{}' is not a builtin source", entry.id);
    };
    if key != "rbac-admin" {
        anyhow::bail!(
            "runtime entry '{}' uses builtin '{}', expected 'rbac-admin'",
            entry.id,
            key
        );
    }

    let permissions = Permissions {
        routes: true,
        commands: false,
        admin: true,
        database: DatabasePermission::Admin,
    };
    ctx.plugins
        .register_builtin_profile_plugin(
            "builtin/rbac-admin",
            "rbac-admin",
            env!("CARGO_PKG_VERSION"),
            "Built-in RBAC Admin routes",
            &permissions,
            entry.enabled,
            entry.required,
        )
        .await;

    let mut staged = ctx.plugins.stage_builtin_activation(entry.id.clone());
    users::register_builtin_capabilities(&mut staged, ctx.clone());
    roles::register_builtin_capabilities(&mut staged, ctx.clone());
    permissions::register_builtin_capabilities(&mut staged, ctx.clone());
    let pending = ctx
        .plugins
        .prepare_owner_activation(staged)
        .await
        .map_err(|error| anyhow::anyhow!(error))?;
    pending.publish().await;
    ctx.plugins.mark_plugin_loaded("rbac-admin", true).await;
    Ok(())
}

pub async fn activate_menu_admin(
    ctx: &SushiContext,
    entry: &ResolvedRuntimeEntry,
) -> anyhow::Result<()> {
    let RuntimePluginSource::Builtin { key, .. } = &entry.source else {
        anyhow::bail!("runtime entry '{}' is not a builtin source", entry.id);
    };
    if key != "menu-admin" {
        anyhow::bail!(
            "runtime entry '{}' uses builtin '{}', expected 'menu-admin'",
            entry.id,
            key
        );
    }

    let permissions = Permissions {
        routes: true,
        commands: false,
        admin: true,
        database: DatabasePermission::Admin,
    };
    ctx.plugins
        .register_builtin_profile_plugin(
            "builtin/menu-admin",
            "menu-admin",
            env!("CARGO_PKG_VERSION"),
            "Built-in Menu Admin routes",
            &permissions,
            entry.enabled,
            entry.required,
        )
        .await;

    let mut staged = ctx.plugins.stage_builtin_activation(entry.id.clone());
    menu::register_menu_admin_capabilities(&mut staged, ctx.clone());
    let pending = ctx
        .plugins
        .prepare_owner_activation(staged)
        .await
        .map_err(|error| anyhow::anyhow!(error))?;
    pending.publish().await;
    ctx.plugins.mark_plugin_loaded("menu-admin", true).await;
    Ok(())
}
