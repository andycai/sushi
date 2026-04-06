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
    /// Run a single plugin
    Run(sushi_cli::commands::run::RunArgs),
    /// Manage plugins
    Plugin(sushi_cli::commands::plugin::PluginArgs),
    /// Manage configuration
    Config(sushi_cli::commands::config_cmd::ConfigArgs),
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Serve(args) => {
            tracing::info!("starting sushi server on {}:{}", args.host, args.port);
            // TODO: load config, init storage, load plugins, build router, serve
            println!("sushi serve placeholder — host={} port={}", args.host, args.port);
        }
        Commands::Run(args) => {
            tracing::info!("running plugin: {}", args.plugin_name);
            // TODO: load plugin and execute
            println!("sushi run placeholder — plugin={}", args.plugin_name);
        }
        Commands::Plugin(args) => match args.command {
            sushi_cli::commands::plugin::PluginCommand::List => {
                // TODO: scan plugins directory
                println!("sushi plugin list placeholder");
            }
        },
        Commands::Config(args) => match args.command {
            sushi_cli::commands::config_cmd::ConfigCommand::Get { key } => {
                println!("sushi config get {} — placeholder", key);
            }
            sushi_cli::commands::config_cmd::ConfigCommand::Set { key, value } => {
                println!("sushi config set {}={} — placeholder", key, value);
            }
        },
    }

    Ok(())
}
