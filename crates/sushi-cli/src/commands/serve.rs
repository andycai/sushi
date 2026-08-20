use anyhow::{Context, Result};
use clap::Args;
use std::path::Path;

#[derive(Args)]
pub struct ServeArgs {
    /// Host to bind (overrides config)
    #[arg(long)]
    pub host: Option<String>,

    /// Port to bind (overrides config)
    #[arg(short, long)]
    pub port: Option<u16>,
}

pub async fn run(
    args: ServeArgs,
    config_path: &Path,
    explicit_profile: Option<&str>,
) -> Result<()> {
    let ctx = crate::app::bootstrap_with_profile(Some(config_path), explicit_profile).await?;
    let result = run_with_context(args, &ctx).await;
    ctx.shutdown().await;
    result
}

pub async fn run_with_context(
    args: ServeArgs,
    ctx: &sushi_core::context::SushiContext,
) -> Result<()> {
    let (host, port) = {
        let cfg = ctx.config.get().await;
        (
            args.host.clone().unwrap_or_else(|| cfg.server.host.clone()),
            args.port.unwrap_or(cfg.server.port),
        )
    };
    tracing::info!("starting sushi server on {}:{}", host, port);

    let app = build_router(&ctx).await;

    let addr = format!("{}:{}", host, port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .context(format!("failed to bind to {addr}"))?;
    tracing::info!("sushi listening on {addr}");
    serve_with_shutdown(listener, app, &ctx, shutdown_signal()).await?;

    Ok(())
}

async fn serve_with_shutdown<F>(
    listener: tokio::net::TcpListener,
    app: axum::Router,
    ctx: &sushi_core::context::SushiContext,
    shutdown: F,
) -> Result<()>
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    let server_result = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await;
    ctx.shutdown().await;
    server_result.context("server error")
}

pub async fn build_router(ctx: &sushi_core::context::SushiContext) -> axum::Router {
    let snapshot = ctx.plugins.capability_snapshot().await;
    let mut app = axum::Router::new()
        .route("/health", axum::routing::get(|| async { "ok" }))
        .merge(sushi_admin::router::build_static_router(ctx).await);
    if snapshot.has_transport(sushi_core::runtime::HttpSurface::Api) {
        app = app.merge(sushi_api::router::build_router(ctx).await);
    }
    if snapshot.has_transport(sushi_core::runtime::HttpSurface::Admin) {
        app = app.merge(sushi_admin::router::build_admin_router(ctx).await);
    }
    app
}

async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            tracing::error!(%error, "failed to install Ctrl-C signal handler");
            std::future::pending::<()>().await;
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(error) => {
                tracing::error!(%error, "failed to install SIGTERM signal handler");
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => tracing::info!("received Ctrl-C; starting graceful shutdown"),
        _ = terminate => tracing::info!("received SIGTERM; starting graceful shutdown"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sushi_core::auth::jwt::JwtService;
    use sushi_core::config::{ConfigStore, SushiConfig};
    use sushi_core::context::SushiContext;
    use sushi_core::storage::sqlite::SqliteStorage;
    use sushi_core::web::template_service::TemplateService;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[tokio::test]
    async fn graceful_shutdown_waits_for_in_flight_http_before_task_cleanup() {
        let templates_dir = tempfile::tempdir().unwrap();
        let ctx = SushiContext::new(
            ConfigStore::new(SushiConfig::default()),
            SqliteStorage::new_in_memory().await.unwrap(),
            JwtService::new("test-secret-key-at-least-32-chars-long!", 3600, 604800),
            TemplateService::new(templates_dir.path()).unwrap(),
        );
        let entered = std::sync::Arc::new(tokio::sync::Notify::new());
        let release = std::sync::Arc::new(tokio::sync::Notify::new());
        let app = axum::Router::new().route(
            "/slow",
            axum::routing::get({
                let entered = std::sync::Arc::clone(&entered);
                let release = std::sync::Arc::clone(&release);
                move || {
                    let entered = std::sync::Arc::clone(&entered);
                    let release = std::sync::Arc::clone(&release);
                    async move {
                        entered.notify_one();
                        release.notified().await;
                        "done"
                    }
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .unwrap();
        let address = listener.local_addr().unwrap();
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let server = tokio::spawn({
            let ctx = ctx.clone();
            async move {
                serve_with_shutdown(listener, app, &ctx, async move {
                    let _ = shutdown_rx.await;
                })
                .await
            }
        });
        let request = tokio::spawn(async move {
            let mut stream = tokio::net::TcpStream::connect(address).await.unwrap();
            stream
                .write_all(b"GET /slow HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
                .await
                .unwrap();
            let mut response = Vec::new();
            stream.read_to_end(&mut response).await.unwrap();
            response
        });

        entered.notified().await;
        shutdown_tx.send(()).unwrap();
        assert!(!server.is_finished());
        release.notify_one();

        let response = request.await.unwrap();
        assert!(String::from_utf8_lossy(&response).contains("200 OK"));
        server.await.unwrap().unwrap();
    }
}
