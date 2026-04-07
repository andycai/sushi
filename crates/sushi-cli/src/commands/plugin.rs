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
}

pub async fn run(args: PluginArgs) -> Result<()> {
    match args.command {
        PluginCommand::List => {
            let ctx = crate::app::bootstrap(None).await?;
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
    }
    Ok(())
}
