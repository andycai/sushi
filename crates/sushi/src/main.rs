use anyhow::Result;
use tracing_subscriber::prelude::*;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    let filter = EnvFilter::from_default_env().add_directive("info".parse()?);
    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .with(sushi_core::logs::tracing_bridge::layer())
        .init();

    sushi_cli::commands::dynamic::run(std::env::args_os().collect()).await
}
