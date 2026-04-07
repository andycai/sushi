use anyhow::{Context, Result};
use axum::{routing::get, Router};
use clap::{Parser, Subcommand};
use sushi_core::auth::password;
use sushi_core::auth::repository::UserRepository;
use sushi_core::auth::model::UserRole;
use sushi_core::config::{ConfigStore, SushiConfig};
use sushi_core::context::SushiContext;
use sushi_core::lua::loader::LuaPlugin;
use sushi_core::plugin::Plugin;
use sushi_core::storage::sqlite::SqliteStorage;
use sushi_core::auth::jwt::JwtService;
use tracing_subscriber::EnvFilter;

/// Embed the initial migration SQL at compile time.
const MIGRATION_SQL: &str = include_str!("../../../migrations/001_init.sql");
const KV_MIGRATION_SQL: &str = include_str!("../../../migrations/002_kv_store.sql");

#[derive(Parser)]
#[command(name = "sushi", version, about = "A modular application platform")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the server
    Serve(sushi_cli::commands::serve::ServeArgs),
    /// Run a single plugin
    Run(sushi_cli::commands::run::RunArgs),
    /// Manage plugins
    Plugin(sushi_cli::commands::plugin::PluginArgs),
    /// Manage configuration
    Config(sushi_cli::commands::config_cmd::ConfigArgs),
    /// Seed the database with an initial admin user
    Seed(sushi_cli::commands::seed::SeedArgs),
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Serve(args) => {
            // Load config (from file or defaults)
            let config = if args.config.exists() {
                ConfigStore::load(&args.config)
                    .await
                    .context("failed to load config")?
            } else {
                tracing::info!("no config file found at {}, using defaults", args.config.display());
                ConfigStore::new(SushiConfig::default())
            };

            // CLI args override config values
            let (host, port, db_path) = {
                let cfg = config.get().await;
                (
                    args.host.clone().unwrap_or_else(|| cfg.server.host.clone()),
                    args.port.unwrap_or(cfg.server.port),
                    cfg.database.path.clone(),
                )
            };
            tracing::info!("starting sushi server on {}:{}", host, port);

            // Ensure the data directory exists
            if let Some(parent) = std::path::Path::new(&db_path).parent() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .context("failed to create database directory")?;
            }

            // Init storage
            let storage = SqliteStorage::new(&db_path)
                .await
                .map_err(|e| anyhow::anyhow!("failed to open database: {e}"))?;

            // Run migrations
            storage
                .run_migrations(MIGRATION_SQL)
                .await
                .map_err(|e| anyhow::anyhow!("failed to run migrations: {e}"))?;
            storage
                .run_migrations(KV_MIGRATION_SQL)
                .await
                .map_err(|e| anyhow::anyhow!("failed to run kv migrations: {e}"))?;

            // Init JWT service
            let jwt = {
                let guard = config.get().await;
                JwtService::new(
                    &guard.jwt.secret,
                    guard.jwt.access_ttl,
                    guard.jwt.refresh_ttl,
                )
            };

            // Build context
            let ctx = SushiContext::new(config.clone(), storage, jwt);

            // Load plugins
            let plugins_dir = {
                let guard = config.get().await;
                std::path::PathBuf::from(&guard.plugins.directory)
            };
            if plugins_dir.exists() {
                let lua_plugins = LuaPlugin::scan_dir(&plugins_dir)
                    .await
                    .context("failed to scan plugins directory")?;
                for plugin in lua_plugins {
                    let plugin_name = plugin.name().to_string();
                    if let Err(e) = plugin.init(&ctx).await {
                        tracing::warn!("failed to init plugin {plugin_name}: {e}");
                        continue;
                    }
                    // Transfer the Lua VM to PluginManager so handlers can be called later
                    if let Some(lua) = plugin.into_vm() {
                        ctx.plugins.register_vm(&plugin_name, lua).await;
                        tracing::debug!("registered VM for plugin {plugin_name}");
                    }
                }
            }

            // Build Axum app
            let api_router = sushi_api::router::build_api_router(&ctx);
            let admin_router = sushi_admin::router::build_admin_router(&ctx).await;

            // Login page at /admin-login (outside auth-protected /admin group)
            let login_router = Router::new()
                .route("/admin-login", get(sushi_admin::routes::login::login_page));

            // Build plugin API routes (dynamic routes from Lua plugins)
            let plugin_api_router = sushi_api::router::build_plugin_api_routes(&ctx)
                .await
                .with_state(ctx.plugins.clone());

            // Admin router uses PluginManager as state for dynamic plugin pages
            let admin_router = admin_router.with_state(ctx.plugins.clone());

            let mut app = axum::Router::new()
                .merge(api_router)
                .merge(login_router)
                .merge(admin_router)
                .merge(plugin_api_router);

            // Conditionally include admin-only or api-only mode
            if args.api_only {
                app = sushi_api::router::build_app(&ctx);
            } else if !args.admin_only {
                // Default: both api and admin
                // (already built above)
            }

            // Serve
            let addr = format!("{}:{}", host, port);
            let listener = tokio::net::TcpListener::bind(&addr)
                .await
                .context(format!("failed to bind to {addr}"))?;
            tracing::info!("sushi listening on {addr}");
            axum::serve(listener, app)
                .await
                .context("server error")?;
        }
        Commands::Run(args) => {
            // Load config, storage, and context to access PluginManager
            let config = ConfigStore::new(SushiConfig::default());
            let db_path = {
                let guard = config.get().await;
                guard.database.path.clone()
            };
            if let Some(parent) = std::path::Path::new(&db_path).parent() {
                tokio::fs::create_dir_all(parent).await.ok();
            }
            let storage = SqliteStorage::new(&db_path)
                .await
                .context("failed to open database")?;
            storage.run_migrations(MIGRATION_SQL)
                .await
                .context("failed to run migrations")?;
            storage.run_migrations(KV_MIGRATION_SQL)
                .await
                .context("failed to run kv migrations")?;

            let jwt = {
                let guard = config.get().await;
                JwtService::new(&guard.jwt.secret, guard.jwt.access_ttl, guard.jwt.refresh_ttl)
            };
            let ctx = SushiContext::new(config, storage, jwt);

            // Load plugins
            let plugins_dir = {
                let guard = ctx.config.get().await;
                std::path::PathBuf::from(&guard.plugins.directory)
            };
            if plugins_dir.exists() {
                let lua_plugins = LuaPlugin::scan_dir(&plugins_dir)
                    .await
                    .context("failed to scan plugins directory")?;
                for plugin in lua_plugins {
                    let plugin_name = plugin.name().to_string();
                    if let Err(e) = plugin.init(&ctx).await {
                        tracing::warn!("failed to init plugin {plugin_name}: {e}");
                        continue;
                    }
                    if let Some(lua) = plugin.into_vm() {
                        ctx.plugins.register_vm(&plugin_name, lua).await;
                    }
                }
            }

            // Dispatch to the CLI handler
            match ctx.plugins.call_cli_handler(&args.plugin_name, &args.args).await {
                Some(Ok(output)) => println!("{output}"),
                Some(Err(e)) => anyhow::bail!("plugin error: {e}"),
                None => anyhow::bail!("command '{}' not found in any plugin", args.plugin_name),
            }
        }
        Commands::Plugin(args) => match args.command {
            sushi_cli::commands::plugin::PluginCommand::List => {
                // TODO: scan plugins directory
                println!("sushi plugin list placeholder");
            }
        },
        Commands::Config(args) => match args.command {
            sushi_cli::commands::config_cmd::ConfigCommand::Get { key } => {
                println!("sushi config get {} — placeholder", key);
            }
            sushi_cli::commands::config_cmd::ConfigCommand::Set { key, value } => {
                println!("sushi config set {}={} — placeholder", key, value);
            }
        },
        Commands::Seed(args) => {
            let config = ConfigStore::new(SushiConfig::default());
            let db_path = {
                let guard = config.get().await;
                guard.database.path.clone()
            };
            if let Some(parent) = std::path::Path::new(&db_path).parent() {
                tokio::fs::create_dir_all(parent).await.ok();
            }
            let storage = SqliteStorage::new(&db_path)
                .await
                .context("failed to open database")?;
            storage.run_migrations(MIGRATION_SQL)
                .await
                .context("failed to run migrations")?;
            storage.run_migrations(KV_MIGRATION_SQL)
                .await
                .context("failed to run kv migrations")?;

            let repo = UserRepository::new(&storage);
            let password_hash = password::hash_password(&args.password)
                .map_err(|e| anyhow::anyhow!("failed to hash password: {e}"))?;

            match repo.create_user(&args.username, &args.email, &password_hash, UserRole::Admin).await {
                Ok(user) => {
                    println!("✓ Admin user created: {} (id={})", user.username, user.id);
                }
                Err(e) => {
                    anyhow::bail!("failed to create user: {e}");
                }
            }
        }
    }

    Ok(())
}
