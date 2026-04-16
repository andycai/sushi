use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing_subscriber::prelude::*;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "sushi", version, about = "A modular application platform")]
struct Cli {
    /// CLI principal role used for command authorization.
    ///
    /// Precedence: --role > SUSHI_CLI_ROLE > admin.
    #[arg(long, global = true)]
    role: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the server
    Serve(sushi_cli::commands::serve::ServeArgs),
    /// Run a single plugin command
    Run(sushi_cli::commands::run::RunArgs),
    /// Manage plugins
    Plugin(sushi_cli::commands::plugin::PluginArgs),
    /// Manage configuration
    Config(sushi_cli::commands::config_cmd::ConfigArgs),
    /// Seed the database with an initial admin user
    Seed(sushi_cli::commands::seed::SeedArgs),
}

#[tokio::main]
async fn main() -> Result<()> {
    let filter = EnvFilter::from_default_env().add_directive("info".parse()?);
    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .with(sushi_core::logs::tracing_bridge::layer())
        .init();

    let cli = Cli::parse();
    let env_role = std::env::var("SUSHI_CLI_ROLE").ok();
    let role =
        sushi_cli::commands::authorization::resolve_cli_role(cli.role.as_deref(), env_role.as_deref());

    match cli.command {
        Commands::Serve(args) => sushi_cli::commands::serve::run(args).await,
        Commands::Run(args) => sushi_cli::commands::run::run(args, &role).await,
        Commands::Plugin(args) => sushi_cli::commands::plugin::run(args, &role).await,
        Commands::Config(args) => sushi_cli::commands::config_cmd::run(args).await,
        Commands::Seed(args) => sushi_cli::commands::seed::run(args).await,
    }
}
