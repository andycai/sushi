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

    // Build the plugin API router (always needed unless admin_only)
    let body_size_limit = {
        let cfg = ctx.config.get().await;
        cfg.server.body_size_limit
    };
    
    let plugin_api_state = sushi_api::router::PluginApiState {
        plugins: ctx.plugins.clone(),
        body_size_limit,
        route_map: vec![],
    };
    
    let plugin_api_router = sushi_api::router::build_plugin_api_routes(&ctx)
        .await
        .with_state(plugin_api_state);

    let app = if args.api_only {
        // API-only: Rust API routes + plugin routes, no admin UI
        // Uses require_auth middleware via build_app, then merge plugin routes
        sushi_api::router::build_app(&ctx).merge(plugin_api_router)
    } else if args.admin_only {
        // Admin-only: admin UI + login page, no API or plugin API routes
        let admin_router = sushi_admin::router::build_admin_router(&ctx).await;
        let login_router = Router::new()
            .route("/admin-login", get(sushi_admin::routes::login::login_page));
        axum::Router::new()
            .merge(login_router)
            .merge(admin_router)
    } else {
        // Default: everything
        let api_router = sushi_api::router::build_api_router(&ctx);
        let admin_router = sushi_admin::router::build_admin_router(&ctx).await;
        let login_router = Router::new()
            .route("/admin-login", get(sushi_admin::routes::login::login_page));
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
