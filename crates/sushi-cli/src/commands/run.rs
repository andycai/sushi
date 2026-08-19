use anyhow::Result;
use clap::Args;
use std::path::Path;

#[derive(Args)]
pub struct RunArgs {
    /// Plugin name to run
    pub plugin_name: String,

    /// Arguments to pass to the plugin
    #[arg(trailing_var_arg = true)]
    pub args: Vec<String>,
}

pub async fn run(
    args: RunArgs,
    role: &str,
    config_path: &Path,
    profile_override: Option<&str>,
) -> Result<()> {
    let ctx = crate::app::bootstrap_with_profile(Some(config_path), profile_override).await?;
    crate::commands::authorization::ensure_command_authorized(&ctx, role, &args.plugin_name)
        .await?;

    match ctx
        .plugins
        .call_cli_handler(&args.plugin_name, &args.args)
        .await
    {
        Some(Ok(output)) => println!("{output}"),
        Some(Err(e)) => {
            if let Some(disabled_error) = map_cli_plugin_error(&args.plugin_name, &e) {
                let warn_message = format!(
                    "plugin CLI command {} blocked because plugin is disabled by administrator",
                    args.plugin_name
                );
                tracing::warn!("{warn_message}");
                ctx.logs.warn(&warn_message).await;
                return Err(disabled_error);
            }
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

fn is_plugin_disabled_error(err: &str) -> bool {
    err.starts_with("plugin_disabled:")
}

fn map_cli_plugin_error(plugin_name: &str, err: &str) -> Option<anyhow::Error> {
    if is_plugin_disabled_error(err) {
        return Some(disabled_command_error(plugin_name));
    }
    None
}

fn disabled_command_error(plugin_name: &str) -> anyhow::Error {
    anyhow::anyhow!("plugin is disabled by administrator: {plugin_name}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_cli_plugin_error_returns_disabled_message() {
        let err = map_cli_plugin_error("cms", "plugin_disabled: plugin 'cms' is disabled")
            .expect("expected disabled plugin mapping");
        assert_eq!(err.to_string(), "plugin is disabled by administrator: cms");
    }
}
