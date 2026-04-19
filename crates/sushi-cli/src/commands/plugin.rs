use anyhow::Result;
use clap::{Args, Subcommand};

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
            crate::commands::authorization::ensure_command_authorized(
                &ctx,
                role,
                "plugin:status",
            )
            .await?;

            let plugins = ctx.plugins.list_plugins().await;
            if let Some(plugin_name) = plugin.as_ref() {
                let Some(item) = plugins.into_iter().find(|p| p.name == *plugin_name) else {
                    anyhow::bail!("plugin not found: {}", plugin_name);
                };
                println!(
                    "{}\t{}\tenabled={}\tloaded={}\tsource_kind={}",
                    item.name, item.version, item.enabled, item.loaded, item.source_kind
                );
            } else {
                for item in plugins {
                    println!(
                        "{}\t{}\tenabled={}\tloaded={}\tsource_kind={}",
                        item.name, item.version, item.enabled, item.loaded, item.source_kind
                    );
                }
            }
        }
        PluginCommand::Enable { plugin, reason } => {
            crate::commands::authorization::ensure_command_authorized(
                &ctx,
                role,
                "plugin:enable",
            )
            .await?;

            let state = ctx
                .plugins
                .set_plugin_enabled(&plugin, true, Some(role), reason.as_deref())
                .await
                .map_err(anyhow::Error::msg)?;
            println!("enabled {} (loaded={})", state.name, state.loaded);
        }
        PluginCommand::Disable { plugin, reason } => {
            crate::commands::authorization::ensure_command_authorized(
                &ctx,
                role,
                "plugin:disable",
            )
            .await?;

            let state = ctx
                .plugins
                .set_plugin_enabled(&plugin, false, Some(role), reason.as_deref())
                .await
                .map_err(anyhow::Error::msg)?;
            println!("disabled {} (loaded={})", state.name, state.loaded);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

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
        let cli = TestCli::try_parse_from([
            "plugin",
            "disable",
            "kv-store",
            "--reason",
            "maintenance",
        ])
        .unwrap();
        match cli.command {
            PluginCommand::Disable { plugin, reason } => {
                assert_eq!(plugin, "kv-store");
                assert_eq!(reason.as_deref(), Some("maintenance"));
            }
            _ => panic!("expected disable command"),
        }
    }
}
