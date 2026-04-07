use anyhow::Result;
use clap::Args;

#[derive(Args)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub command: ConfigCommand,
}

#[derive(clap::Subcommand)]
pub enum ConfigCommand {
    /// Get a config value
    Get {
        /// Config key
        key: String,
    },
    /// Set a config value
    Set {
        /// Config key
        key: String,
        /// Config value
        value: String,
    },
}

pub async fn run(args: ConfigArgs) -> Result<()> {
    match args.command {
        ConfigCommand::Get { key } => {
            // TODO: implement config read from file
            println!("sushi config get {} — placeholder", key);
        }
        ConfigCommand::Set { key, value } => {
            // TODO: implement config write to file
            println!("sushi config set {}={} — placeholder", key, value);
        }
    }
    Ok(())
}
