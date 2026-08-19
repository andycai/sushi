use anyhow::Result;
use clap::Args;
use std::path::Path;
use sushi_core::auth::model::UserRole;
use sushi_core::auth::password;
use sushi_core::auth::repository::UserRepository;

#[derive(Args)]
pub struct SeedArgs {
    /// Username for the admin user
    #[arg(long, default_value = "admin")]
    pub username: String,

    /// Password for the admin user
    #[arg(long, default_value = "admin123")]
    pub password: String,

    /// Email for the admin user
    #[arg(long, default_value = "admin@sushi.local")]
    pub email: String,
}

pub async fn run(args: SeedArgs, config_path: &Path, profile_override: Option<&str>) -> Result<()> {
    let ctx = crate::app::bootstrap_with_profile(Some(config_path), profile_override).await?;
    let repo = UserRepository::new(ctx.db.clone());
    let password_hash = password::hash_password(&args.password)
        .map_err(|e| anyhow::anyhow!("failed to hash password: {e}"))?;

    match repo
        .create_user(&args.username, &args.email, &password_hash, UserRole::Admin)
        .await
    {
        Ok(user) => println!("✓ Admin user created: {} (id={})", user.username, user.id),
        Err(e) => anyhow::bail!("failed to create user: {e}"),
    }

    Ok(())
}
