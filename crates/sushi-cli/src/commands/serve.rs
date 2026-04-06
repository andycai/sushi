use clap::Args;
use std::path::PathBuf;

#[derive(Args)]
pub struct ServeArgs {
    /// Host to bind
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,

    /// Port to bind
    #[arg(short, long, default_value_t = 3000)]
    pub port: u16,

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
