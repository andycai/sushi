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

    /// Only start API server
    #[arg(long)]
    pub api_only: bool,

    /// Only start Admin server
    #[arg(long)]
    pub admin_only: bool,
}

pub async fn run(
    args: ServeArgs,
    config_path: &Path,
    explicit_profile: Option<&str>,
) -> Result<()> {
    let profile_override =
        resolve_profile_override(explicit_profile, args.api_only, args.admin_only)?;
    if args.api_only {
        tracing::warn!("--api-only is deprecated; use --profile api");
    } else if args.admin_only {
        tracing::warn!("--admin-only is deprecated; use --profile admin");
    }
    let ctx =
        crate::app::bootstrap_with_profile(Some(config_path), profile_override.as_deref()).await?;
    let include_api = ctx.runtime_profile.has_enabled_builtin("identity")
        || ctx.runtime_profile.has_enabled_builtin("api-core");
    let include_admin = ctx.runtime_profile.has_enabled_builtin("host-admin");

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
        auth_state: ctx.auth_state(),
        logs: ctx.logs.clone(),
        body_size_limit,
    };

    let mut app = axum::Router::new().route("/health", axum::routing::get(|| async { "ok" }));
    if include_api {
        let plugin_api_router = sushi_api::router::build_plugin_api_routes(&ctx)
            .await
            .with_state(plugin_api_state);
        app = app
            .merge(sushi_api::router::build_app(&ctx))
            .merge(plugin_api_router);
    }
    if include_admin {
        app = app.merge(sushi_admin::router::build_admin_router(&ctx).await);
    }

    let addr = format!("{}:{}", host, port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .context(format!("failed to bind to {addr}"))?;
    tracing::info!("sushi listening on {addr}");
    axum::serve(listener, app).await.context("server error")?;

    Ok(())
}

fn resolve_profile_override(
    explicit_profile: Option<&str>,
    api_only: bool,
    admin_only: bool,
) -> Result<Option<String>> {
    if api_only && admin_only {
        anyhow::bail!("--api-only and --admin-only cannot be used together");
    }
    if api_only {
        return Ok(Some("api".to_string()));
    }
    if admin_only {
        return Ok(Some("admin".to_string()));
    }
    Ok(explicit_profile.map(str::to_string))
}

#[cfg(test)]
mod tests {
    use super::resolve_profile_override;

    #[test]
    fn legacy_surface_flags_map_to_profiles() {
        assert_eq!(
            resolve_profile_override(None, true, false)
                .unwrap()
                .as_deref(),
            Some("api")
        );
        assert_eq!(
            resolve_profile_override(None, false, true)
                .unwrap()
                .as_deref(),
            Some("admin")
        );
    }

    #[test]
    fn legacy_surface_flags_override_explicit_profile() {
        assert_eq!(
            resolve_profile_override(Some("minimal"), true, false)
                .unwrap()
                .as_deref(),
            Some("api")
        );
    }

    #[test]
    fn conflicting_legacy_surface_flags_are_rejected() {
        assert!(resolve_profile_override(None, true, true).is_err());
    }
}
