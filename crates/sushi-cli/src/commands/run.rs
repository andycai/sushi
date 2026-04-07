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

pub async fn run(args: RunArgs) -> Result<()> {
    let ctx = crate::app::bootstrap(None).await?;

    match ctx
        .plugins
        .call_cli_handler(&args.plugin_name, &args.args)
        .await
    {
        Some(Ok(output)) => println!("{output}"),
        Some(Err(e)) => anyhow::bail!("plugin error: {e}"),
        None => anyhow::bail!("command '{}' not found in any plugin", args.plugin_name),
    }

    Ok(())
}
