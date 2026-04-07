use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "sushi", version, about = "A modular application platform")]
struct Cli {
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
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Serve(args) => sushi_cli::commands::serve::run(args).await,
        Commands::Run(args) => sushi_cli::commands::run::run(args).await,
        Commands::Plugin(args) => sushi_cli::commands::plugin::run(args).await,
        Commands::Config(args) => sushi_cli::commands::config_cmd::run(args).await,
        Commands::Seed(args) => sushi_cli::commands::seed::run(args).await,
    }
}
