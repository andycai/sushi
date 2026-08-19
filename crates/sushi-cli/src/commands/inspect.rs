use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use std::path::Path;

#[derive(Args)]
pub struct InspectArgs {
    #[command(subcommand)]
    pub command: InspectCommand,
}

#[derive(Subcommand)]
pub enum InspectCommand {
    /// Dump the resolved profile without opening the database.
    Profile,
    /// Dump the resolved profile and active capability registrations.
    Capabilities,
}

pub async fn run(
    args: InspectArgs,
    config_path: &Path,
    profile_override: Option<&str>,
) -> Result<()> {
    match args.command {
        InspectCommand::Profile => {
            let (_config, profile) =
                crate::app::resolve_runtime_profile(Some(config_path), profile_override).await?;
            println!("{}", profile.dump_json()?);
        }
        InspectCommand::Capabilities => {
            let ctx = crate::app::bootstrap_with_profile(Some(config_path), profile_override)
                .await
                .context("failed to bootstrap runtime for capability inspection")?;
            println!("{}", ctx.runtime_profile.dump_json()?);
            println!("\n[capabilities]");
            for entry in ctx.plugins.capability_snapshot().await.inspect() {
                println!("{}\towner={}", entry.key, entry.owner);
            }
        }
    }

    Ok(())
}
