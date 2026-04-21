use anyhow::{Error, Result};
use clap::{Args, Subcommand};
use sushi_core::plugin::manager::PluginInfo;

#[derive(Args)]
pub struct PluginArgs {
    #[command(subcommand)]
    pub command: PluginCommand,
}

#[derive(Subcommand)]
pub enum PluginCommand {
    /// List all discovered plugins
    List,
    /// Show plugin status
    Status {
        /// Plugin name (optional)
        plugin: Option<String>,
    },
    /// Enable plugin runtime dispatch
    Enable {
        /// Plugin name
        plugin: String,
        /// Optional reason
        #[arg(long)]
        reason: Option<String>,
    },
    /// Disable plugin runtime dispatch
    Disable {
        /// Plugin name
        plugin: String,
        /// Optional reason
        #[arg(long)]
        reason: Option<String>,
    },
}

pub async fn run(args: PluginArgs, role: &str) -> Result<()> {
    let ctx = crate::app::bootstrap(None).await?;

    match args.command {
        PluginCommand::List => {
            crate::commands::authorization::ensure_command_authorized(&ctx, role, "plugin:list")
                .await?;
            let routes = ctx.plugins.list_api_routes().await;
            let cmds = ctx.plugins.list_cli_commands().await;
            let pages = ctx.plugins.list_admin_pages().await;

            println!("=== API Routes ({}) ===", routes.len());
            for (method, path) in &routes {
                println!("  {} {}", method, path);
            }
            println!("\n=== CLI Commands ({}) ===", cmds.len());
            for cmd in &cmds {
                println!("  {}", cmd);
            }
            println!("\n=== Admin Pages ({}) ===", pages.len());
            for page in &pages {
                println!("  {}", page);
            }
        }
        PluginCommand::Status { plugin } => {
            crate::commands::authorization::ensure_command_authorized(&ctx, role, "plugin:status")
                .await?;

            let targets =
                select_status_targets(ctx.plugins.list_plugins().await, plugin.as_deref())?;
            for item in targets {
                println!(
                    "{}\t{}\tenabled={}\tloaded={}\tsource_kind={}",
                    item.name, item.version, item.enabled, item.loaded, item.source_kind
                );
            }
        }
        PluginCommand::Enable { plugin, reason } => {
            crate::commands::authorization::ensure_command_authorized(&ctx, role, "plugin:enable")
                .await?;

            let state = ctx
                .plugins
                .set_plugin_enabled(&plugin, true, Some(role), reason.as_deref())
                .await
                .map_err(map_toggle_error)?;
            println!("enabled {} (loaded={})", state.name, state.loaded);
        }
        PluginCommand::Disable { plugin, reason } => {
            crate::commands::authorization::ensure_command_authorized(&ctx, role, "plugin:disable")
                .await?;

            let state = ctx
                .plugins
                .set_plugin_enabled(&plugin, false, Some(role), reason.as_deref())
                .await
                .map_err(map_toggle_error)?;
            println!("disabled {} (loaded={})", state.name, state.loaded);
        }
    }
    Ok(())
}

fn select_status_targets(
    plugins: Vec<PluginInfo>,
    target: Option<&str>,
) -> Result<Vec<PluginInfo>> {
    if let Some(plugin_name) = target {
        let Some(item) = plugins.into_iter().find(|p| p.name == plugin_name) else {
            anyhow::bail!("plugin not found: {}", plugin_name);
        };
        return Ok(vec![item]);
    }

    Ok(plugins)
}

fn map_toggle_error(err: String) -> Error {
    Error::msg(err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use sushi_core::plugin::manager::PluginPermissionsView;

    #[derive(Parser)]
    struct TestCli {
        #[command(subcommand)]
        command: PluginCommand,
    }

    #[test]
    fn parse_enable_subcommand() {
        let cli = TestCli::try_parse_from(["plugin", "enable", "kv-store"]).unwrap();
        match cli.command {
            PluginCommand::Enable { plugin, .. } => assert_eq!(plugin, "kv-store"),
            _ => panic!("expected enable command"),
        }
    }

    #[test]
    fn parse_status_subcommand() {
        let cli = TestCli::try_parse_from(["plugin", "status", "kv-store"]).unwrap();
        match cli.command {
            PluginCommand::Status { plugin } => {
                assert_eq!(plugin.as_deref(), Some("kv-store"));
            }
            _ => panic!("expected status command"),
        }
    }

    #[test]
    fn parse_disable_subcommand() {
        let cli =
            TestCli::try_parse_from(["plugin", "disable", "kv-store", "--reason", "maintenance"])
                .unwrap();
        match cli.command {
            PluginCommand::Disable { plugin, reason } => {
                assert_eq!(plugin, "kv-store");
                assert_eq!(reason.as_deref(), Some("maintenance"));
            }
            _ => panic!("expected disable command"),
        }
    }

    #[test]
    fn status_selection_returns_error_for_missing_plugin() {
        let plugins = vec![test_plugin("kv-store"), test_plugin("notes")];
        let err = select_status_targets(plugins, Some("missing")).unwrap_err();
        assert_eq!(err.to_string(), "plugin not found: missing");
    }

    #[test]
    fn status_selection_returns_single_match_for_named_plugin() {
        let plugins = vec![test_plugin("kv-store"), test_plugin("notes")];
        let selected = select_status_targets(plugins, Some("notes")).unwrap();
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].name, "notes");
    }

    #[test]
    fn map_toggle_error_preserves_message() {
        let err = map_toggle_error("plugin not found: missing".to_string());
        assert_eq!(err.to_string(), "plugin not found: missing");
    }

    fn test_plugin(name: &str) -> PluginInfo {
        PluginInfo {
            plugin_id: format!("plugin-{name}"),
            source_kind: "third_party".to_string(),
            name: name.to_string(),
            version: "0.1.0".to_string(),
            description: String::new(),
            enabled: true,
            loaded: true,
            permissions: PluginPermissionsView {
                routes: true,
                commands: true,
                admin: true,
                database: "none".to_string(),
            },
        }
    }
}
