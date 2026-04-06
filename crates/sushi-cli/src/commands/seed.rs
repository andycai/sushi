use clap::Args;

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
