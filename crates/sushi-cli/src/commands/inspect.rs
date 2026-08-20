use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use std::path::{Path, PathBuf};
use sushi_core::context::SushiContext;

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
    run_with_overlays(args, config_path, profile_override, &[]).await
}

pub async fn run_with_overlays(
    args: InspectArgs,
    config_path: &Path,
    profile_override: Option<&str>,
    overlay_paths: &[PathBuf],
) -> Result<()> {
    match args.command {
        InspectCommand::Profile => {
            let (_config, profile) = crate::app::resolve_runtime_profile_with_overlays(
                Some(config_path),
                profile_override,
                overlay_paths,
            )
            .await?;
            println!("{}", profile.dump_json()?);
        }
        InspectCommand::Capabilities => {
            let ctx = crate::app::bootstrap_with_profile_and_overlays(
                Some(config_path),
                profile_override,
                overlay_paths,
            )
            .await
            .context("failed to bootstrap runtime for capability inspection")?;
            let result = print_capabilities(&ctx).await;
            ctx.shutdown().await;
            result?;
        }
    }

    Ok(())
}

pub async fn run_with_context(args: InspectArgs, ctx: &SushiContext) -> Result<()> {
    match args.command {
        InspectCommand::Profile => println!("{}", ctx.runtime_profile.dump_json()?),
        InspectCommand::Capabilities => print_capabilities(ctx).await?,
    }
    Ok(())
}

async fn print_capabilities(ctx: &SushiContext) -> Result<()> {
    println!("{}", ctx.runtime_profile.dump_json()?);
    println!("\n[capabilities]");
    for entry in ctx.plugins.capability_snapshot().await.inspect() {
        println!(
            "{}\towner={}\tsource={}",
            entry.key, entry.owner, entry.source
        );
    }
    Ok(())
}
