use async_trait::async_trait;
use clap::{Args as ClapArgs, Command};
use sushi_core::context::{PluginContext, SushiContext};
use sushi_core::plugin::{DatabasePermission, Permissions};
use sushi_core::runtime::{
    BuiltinPluginFactory, CliCommandSpec, CliHandler, ResolvedRuntimeEntry, RuntimePluginSource,
};

pub struct HostCliFactory {
    role: String,
}

impl HostCliFactory {
    pub fn new(role: impl Into<String>) -> Self {
        Self { role: role.into() }
    }
}

#[async_trait]
impl BuiltinPluginFactory for HostCliFactory {
    fn key(&self) -> &'static str {
        "host-cli"
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
                "builtin/host-cli",
                "host-cli",
                env!("CARGO_PKG_VERSION"),
                "Built-in dynamic CLI host",
                &Permissions {
                    routes: false,
                    commands: true,
                    admin: false,
                    database: DatabasePermission::None,
                },
                entry.enabled,
                entry.required,
            )
            .await;

        let mut staged = ctx.plugins.stage_builtin_activation(entry.id.clone());
        register_commands(&mut staged, ctx, &self.role);
        let pending = ctx
            .plugins
            .prepare_owner_activation(staged)
            .await
            .map_err(anyhow::Error::msg)?;
        pending.publish().await;
        ctx.plugins.mark_plugin_loaded("host-cli", true).await;
        Ok(())
    }
}

fn register_commands(
    staged: &mut sushi_core::runtime::StagedRegistrar,
    ctx: &SushiContext,
    role: &str,
) {
    let runtime_ctx = ctx.clone();
    staged.register_cli(
        CliCommandSpec::new("serve", "Start the server", "host-cli", "builtin::serve")
            .with_rust_handler(CliHandler::new({
                let runtime_ctx = runtime_ctx.clone();
                move |args| {
                    let runtime_ctx = runtime_ctx.clone();
                    async move {
                        let parsed =
                            parse_args::<crate::commands::serve::ServeArgs>("serve", &args)?;
                        crate::commands::serve::run_with_context(parsed, &runtime_ctx)
                            .await
                            .map(|_| String::new())
                            .map_err(|error| error.to_string())
                    }
                }
            })),
    );

    staged.register_cli(
        CliCommandSpec::new("plugin", "Manage plugins", "host-cli", "builtin::plugin")
            .with_rust_handler(CliHandler::new({
                let role = role.to_string();
                let runtime_ctx = runtime_ctx.clone();
                move |args| {
                    let role = role.clone();
                    let runtime_ctx = runtime_ctx.clone();
                    async move {
                        let parsed =
                            parse_args::<crate::commands::plugin::PluginArgs>("plugin", &args)?;
                        crate::commands::plugin::run_with_context(parsed, &role, &runtime_ctx)
                            .await
                            .map(|_| String::new())
                            .map_err(|error| error.to_string())
                    }
                }
            })),
    );

    staged.register_cli(
        CliCommandSpec::new(
            "config",
            "Manage configuration",
            "host-cli",
            "builtin::config",
        )
        .with_rust_handler(CliHandler::new(move |args| async move {
            let parsed = parse_args::<crate::commands::config_cmd::ConfigArgs>("config", &args)?;
            crate::commands::config_cmd::run(parsed)
                .await
                .map(|_| String::new())
                .map_err(|error| error.to_string())
        })),
    );

    staged.register_cli(
        CliCommandSpec::new(
            "seed",
            "Seed the database with an initial admin user",
            "host-cli",
            "builtin::seed",
        )
        .with_rust_handler(CliHandler::new({
            let runtime_ctx = runtime_ctx.clone();
            move |args| {
                let runtime_ctx = runtime_ctx.clone();
                async move {
                    let parsed = parse_args::<crate::commands::seed::SeedArgs>("seed", &args)?;
                    crate::commands::seed::run_with_context(parsed, &runtime_ctx)
                        .await
                        .map(|_| String::new())
                        .map_err(|error| error.to_string())
                }
            }
        })),
    );

    staged.register_cli(
        CliCommandSpec::new(
            "inspect",
            "Inspect resolved runtime state",
            "host-cli",
            "builtin::inspect",
        )
        .with_rust_handler(CliHandler::new({
            let runtime_ctx = runtime_ctx.clone();
            move |args| {
                let runtime_ctx = runtime_ctx.clone();
                async move {
                    let parsed =
                        parse_args::<crate::commands::inspect::InspectArgs>("inspect", &args)?;
                    crate::commands::inspect::run_with_context(parsed, &runtime_ctx)
                        .await
                        .map(|_| String::new())
                        .map_err(|error| error.to_string())
                }
            }
        })),
    );
}

pub(crate) fn parse_args<T>(name: &str, args: &[String]) -> Result<T, String>
where
    T: ClapArgs,
{
    let mut argv = Vec::with_capacity(args.len() + 1);
    argv.push(name.to_string());
    argv.extend(args.iter().cloned());
    let matches = T::augment_args(Command::new(name.to_string()).disable_help_flag(true))
        .try_get_matches_from(argv)
        .map_err(|error| error.to_string())?;
    T::from_arg_matches(&matches).map_err(|error| error.to_string())
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
