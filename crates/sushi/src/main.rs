use anyhow::{Context, Result};
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
            tracing::info!("starting sushi server on {}:{}", args.host, args.port);

            // Load config (from file or defaults)
            let config = if args.config.exists() {
                ConfigStore::load(&args.config)
                    .await
                    .context("failed to load config")?
            } else {
                tracing::info!("no config file found at {}, using defaults", args.config.display());
                ConfigStore::new(SushiConfig::default())
            };

            // Read config values needed below
            let db_path = {
                let guard = config.get().await;
                guard.database.path.clone()
            };

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
                for plugin in &lua_plugins {
                    if let Err(e) = plugin.init(&ctx).await {
                        tracing::warn!("failed to init plugin {}: {e}", plugin.name());
                    }
                }
            }

            // Build Axum app
            let api_router = sushi_api::router::build_api_router(&ctx);
            let admin_router = sushi_admin::router::build_admin_router();

            let mut app = axum::Router::new()
                .merge(api_router)
                .nest("/admin", admin_router);

            // Conditionally include admin-only or api-only mode
            if args.api_only {
                app = sushi_api::router::build_app(&ctx);
            } else if !args.admin_only {
                // Default: both api and admin
                // (already built above)
            }

            // Serve
            let addr = format!("{}:{}", args.host, args.port);
            let listener = tokio::net::TcpListener::bind(&addr)
                .await
                .context(format!("failed to bind to {addr}"))?;
            tracing::info!("sushi listening on {addr}");
            axum::serve(listener, app)
                .await
                .context("server error")?;
        }
        Commands::Run(args) => {
            tracing::info!("running plugin: {}", args.plugin_name);
            // TODO: load plugin and execute
            println!("sushi run placeholder — plugin={}", args.plugin_name);
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
