use clap::Args;
use std::path::PathBuf;

#[derive(Args)]
pub struct ServeArgs {
    /// Host to bind (overrides config)
    #[arg(long)]
    pub host: Option<String>,

    /// Port to bind (overrides config)
    #[arg(short, long)]
    pub port: Option<u16>,

    /// Only start API server
    #[arg(long)]
    pub api_only: bool,

    /// Only start Admin server
    #[arg(long)]
    pub admin_only: bool,

    /// Config file path
    #[arg(short, long, default_value = "config.toml")]
    pub config: PathBuf,

    /// Dev mode: serve admin UI from local ui/src/ directory
    #[arg(long)]
    pub admin_dev: bool,
}
