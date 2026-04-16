use anyhow::Result;
use clap::Args;

#[derive(Args)]
pub struct RunArgs {
    /// Plugin name to run
    pub plugin_name: String,

    /// Arguments to pass to the plugin
    #[arg(trailing_var_arg = true)]
    pub args: Vec<String>,
}

pub async fn run(args: RunArgs, role: &str) -> Result<()> {
    let ctx = crate::app::bootstrap(None).await?;
    crate::commands::authorization::ensure_command_authorized(&ctx, role, &args.plugin_name)
        .await?;

    match ctx
        .plugins
        .call_cli_handler(&args.plugin_name, &args.args)
        .await
    {
        Some(Ok(output)) => println!("{output}"),
        Some(Err(e)) => {
            tracing::error!(
                "plugin runtime error on CLI command {}: {e}",
                args.plugin_name
            );
            ctx.logs
                .error(&format!(
                    "plugin runtime error on CLI command {}: {e}",
                    args.plugin_name
                ))
                .await;
            anyhow::bail!("plugin error: {e}")
        }
        None => anyhow::bail!("command '{}' not found in any plugin", args.plugin_name),
    }

    Ok(())
}
