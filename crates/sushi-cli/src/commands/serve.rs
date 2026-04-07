use anyhow::{Context, Result};
use axum::{routing::get, Router};
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

pub async fn run(args: ServeArgs) -> Result<()> {
    let ctx = crate::app::bootstrap(Some(&args.config)).await?;

    let (host, port) = {
        let cfg = ctx.config.get().await;
        (
            args.host.clone().unwrap_or_else(|| cfg.server.host.clone()),
            args.port.unwrap_or(cfg.server.port),
        )
    };
    tracing::info!("starting sushi server on {}:{}", host, port);

    let api_router = sushi_api::router::build_api_router(&ctx);
    let admin_router = sushi_admin::router::build_admin_router(&ctx).await;
    let login_router = Router::new()
        .route("/admin-login", get(sushi_admin::routes::login::login_page));
    let plugin_api_router = sushi_api::router::build_plugin_api_routes(&ctx)
        .await
        .with_state(ctx.plugins.clone());
    let admin_router = admin_router.with_state(ctx.plugins.clone());

    let app = if args.api_only {
        sushi_api::router::build_app(&ctx)
    } else {
        axum::Router::new()
            .merge(api_router)
            .merge(login_router)
            .merge(admin_router)
            .merge(plugin_api_router)
    };

    let addr = format!("{}:{}", host, port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .context(format!("failed to bind to {addr}"))?;
    tracing::info!("sushi listening on {addr}");
    axum::serve(listener, app)
        .await
        .context("server error")?;

    Ok(())
}
