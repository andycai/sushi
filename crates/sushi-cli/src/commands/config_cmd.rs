use anyhow::{bail, Context, Result};
use clap::Args;
use std::fs;
use std::io::Write;
use std::path::Path;
use sushi_core::context::SushiContext;
use toml_edit::{value, DocumentMut, Item, Table};

#[derive(Args)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub command: ConfigCommand,
}

#[derive(clap::Subcommand)]
pub enum ConfigCommand {
    /// Get a config value
    Get {
        /// Config key
        key: String,
    },
    /// Set a config value
    Set {
        /// Config key
        key: String,
        /// Config value
        value: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConfigKey {
    ServerHost,
    ServerPort,
    ServerBodySizeLimit,
    DatabasePath,
    JwtSecret,
    JwtAccessTtl,
    JwtRefreshTtl,
    PluginsDirectory,
    FileBrowserRootDir,
    WebTemplatesDir,
    WebStaticDir,
    WebStaticUrlPrefix,
    RuntimeProfile,
    RuntimeProfilesDir,
    RuntimeBundlesDir,
}

impl ConfigKey {
    fn parse(input: &str) -> Result<Self> {
        match input {
            "server.host" => Ok(Self::ServerHost),
            "server.port" => Ok(Self::ServerPort),
            "server.body_size_limit" => Ok(Self::ServerBodySizeLimit),
            "database.path" => Ok(Self::DatabasePath),
            "jwt.secret" => Ok(Self::JwtSecret),
            "jwt.access_ttl" => Ok(Self::JwtAccessTtl),
            "jwt.refresh_ttl" => Ok(Self::JwtRefreshTtl),
            "plugins.directory" => Ok(Self::PluginsDirectory),
            "file_browser.root_dir" => Ok(Self::FileBrowserRootDir),
            "web.templates_dir" => Ok(Self::WebTemplatesDir),
            "web.static_dir" => Ok(Self::WebStaticDir),
            "web.static_url_prefix" => Ok(Self::WebStaticUrlPrefix),
            "runtime.profile" => Ok(Self::RuntimeProfile),
            "runtime.profiles_dir" => Ok(Self::RuntimeProfilesDir),
            "runtime.bundles_dir" => Ok(Self::RuntimeBundlesDir),
            _ => bail!("unknown config key '{input}'"),
        }
    }
}

pub async fn run_with_context(
    args: ConfigArgs,
    config_path: &Path,
    ctx: &SushiContext,
) -> Result<String> {
    match args.command {
        ConfigCommand::Get { key } => {
            let key = ConfigKey::parse(&key)?;
            let config = ctx.config.get().await;
            read_value(&config, key)
        }
        ConfigCommand::Set { key, value } => {
            let key = ConfigKey::parse(&key)?;
            if key == ConfigKey::JwtSecret {
                bail!("config key 'jwt.secret' is sensitive and cannot be managed through CLI arguments");
            }
            let config_path = config_path.to_path_buf();
            let update_path = config_path.clone();
            tokio::task::spawn_blocking(move || update_file(&update_path, key, &value))
                .await
                .context("config update task failed")??;
            Ok(format!(
                "updated {key} in {}; restart Sushi to apply changes",
                config_path.display()
            ))
        }
    }
}

fn read_value(config: &sushi_core::config::SushiConfig, key: ConfigKey) -> Result<String> {
    let value = match key {
        ConfigKey::ServerHost => toml_string(&config.server.host),
        ConfigKey::ServerPort => config.server.port.to_string(),
        ConfigKey::ServerBodySizeLimit => config.server.body_size_limit.to_string(),
        ConfigKey::DatabasePath => toml_string(&config.database.path),
        ConfigKey::JwtSecret => {
            bail!("config key 'jwt.secret' is sensitive and cannot be displayed")
        }
        ConfigKey::JwtAccessTtl => config.jwt.access_ttl.to_string(),
        ConfigKey::JwtRefreshTtl => config.jwt.refresh_ttl.to_string(),
        ConfigKey::PluginsDirectory => toml_string(&config.plugins.directory),
        ConfigKey::FileBrowserRootDir => toml_string(&config.file_browser.root_dir),
        ConfigKey::WebTemplatesDir => toml_string(&config.web.templates_dir),
        ConfigKey::WebStaticDir => toml_string(&config.web.static_dir),
        ConfigKey::WebStaticUrlPrefix => toml_string(&config.web.static_url_prefix),
        ConfigKey::RuntimeProfile => config
            .runtime
            .profile
            .as_deref()
            .map(toml_string)
            .unwrap_or_else(|| "null".to_string()),
        ConfigKey::RuntimeProfilesDir => toml_string(&config.runtime.profiles_dir),
        ConfigKey::RuntimeBundlesDir => toml_string(&config.runtime.bundles_dir),
    };
    Ok(value)
}

fn toml_string(value: &str) -> String {
    toml::Value::String(value.to_string()).to_string()
}

fn update_file(path: &Path, key: ConfigKey, raw_value: &str) -> Result<()> {
    let original = fs::read_to_string(path)
        .with_context(|| format!("failed to read config {}", path.display()))?;
    let mut document = original
        .parse::<DocumentMut>()
        .with_context(|| format!("failed to parse config {}", path.display()))?;
    apply_update(&mut document, key, raw_value)?;

    let updated = document.to_string();
    toml::from_str::<sushi_core::config::SushiConfig>(&updated)
        .with_context(|| format!("updated config {} is invalid", path.display()))?;
    persist_atomically(path, updated.as_bytes())
}

fn apply_update(document: &mut DocumentMut, key: ConfigKey, raw_value: &str) -> Result<()> {
    let (section, field) = key.path();
    match key.parse_update(raw_value)? {
        ConfigUpdate::Set(item) => {
            if document.get(section).is_none() {
                document[section] = Item::Table(Table::new());
            }
            document[section][field] = item;
        }
        ConfigUpdate::Remove => {
            if let Some(table) = document.get_mut(section).and_then(Item::as_table_mut) {
                table.remove(field);
            }
        }
    }
    Ok(())
}

fn persist_atomically(path: &Path, content: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let existing_permissions = fs::metadata(path)
        .ok()
        .map(|metadata| metadata.permissions());
    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .with_context(|| format!("failed to create temporary config in {}", parent.display()))?;
    temporary
        .write_all(content)
        .with_context(|| format!("failed to write temporary config for {}", path.display()))?;
    temporary
        .as_file_mut()
        .sync_all()
        .with_context(|| format!("failed to sync temporary config for {}", path.display()))?;
    if let Some(permissions) = existing_permissions {
        temporary
            .as_file()
            .set_permissions(permissions)
            .with_context(|| format!("failed to preserve permissions for {}", path.display()))?;
    }
    temporary
        .persist(path)
        .map_err(|error| error.error)
        .with_context(|| format!("failed to replace config {}", path.display()))?;
    Ok(())
}

enum ConfigUpdate {
    Set(Item),
    Remove,
}

impl ConfigKey {
    fn path(self) -> (&'static str, &'static str) {
        match self {
            Self::ServerHost => ("server", "host"),
            Self::ServerPort => ("server", "port"),
            Self::ServerBodySizeLimit => ("server", "body_size_limit"),
            Self::DatabasePath => ("database", "path"),
            Self::JwtSecret => ("jwt", "secret"),
            Self::JwtAccessTtl => ("jwt", "access_ttl"),
            Self::JwtRefreshTtl => ("jwt", "refresh_ttl"),
            Self::PluginsDirectory => ("plugins", "directory"),
            Self::FileBrowserRootDir => ("file_browser", "root_dir"),
            Self::WebTemplatesDir => ("web", "templates_dir"),
            Self::WebStaticDir => ("web", "static_dir"),
            Self::WebStaticUrlPrefix => ("web", "static_url_prefix"),
            Self::RuntimeProfile => ("runtime", "profile"),
            Self::RuntimeProfilesDir => ("runtime", "profiles_dir"),
            Self::RuntimeBundlesDir => ("runtime", "bundles_dir"),
        }
    }

    fn parse_update(self, input: &str) -> Result<ConfigUpdate> {
        let update = match self {
            Self::ServerPort => ConfigUpdate::Set(value(
                input
                    .parse::<u16>()
                    .with_context(|| format!("invalid value for {self}: expected port 0-65535"))?
                    as i64,
            )),
            Self::ServerBodySizeLimit => ConfigUpdate::Set(value(parse_usize(self, input)?)),
            Self::JwtAccessTtl | Self::JwtRefreshTtl => ConfigUpdate::Set(value(
                input
                    .parse::<i64>()
                    .with_context(|| format!("invalid value for {self}: expected integer"))?,
            )),
            Self::RuntimeProfile if input == "null" => ConfigUpdate::Remove,
            _ => ConfigUpdate::Set(value(parse_non_empty(self, input)?)),
        };
        Ok(update)
    }
}

fn parse_usize(key: ConfigKey, input: &str) -> Result<i64> {
    let value = input
        .parse::<usize>()
        .with_context(|| format!("invalid value for {key}: expected non-negative integer"))?;
    i64::try_from(value).with_context(|| format!("invalid value for {key}: integer is too large"))
}

fn parse_non_empty(key: ConfigKey, input: &str) -> Result<String> {
    if input.trim().is_empty() {
        bail!("invalid value for {key}: value must not be empty");
    }
    Ok(input.to_string())
}

impl std::fmt::Display for ConfigKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (section, field) = self.path();
        write!(formatter, "{section}.{field}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_file_does_not_create_a_missing_config() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("missing.toml");
        let error = update_file(&path, ConfigKey::ServerPort, "4100").unwrap_err();
        assert!(error.to_string().contains("failed to read config"));
        assert!(!path.exists());
    }

    #[test]
    fn update_file_creates_a_missing_section_in_an_existing_config() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("config.toml");
        fs::write(&path, "# host config\n").unwrap();

        update_file(&path, ConfigKey::ServerPort, "4100").unwrap();

        let updated = fs::read_to_string(&path).unwrap();
        assert!(updated.contains("# host config"));
        assert!(updated.contains("[server]"));
        assert!(updated.contains("port = 4100"));
    }

    #[test]
    fn update_file_rejects_blank_strings_without_modifying_config() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("config.toml");
        let original = "[server]\nhost = \"127.0.0.1\"\n";
        fs::write(&path, original).unwrap();

        let error = update_file(&path, ConfigKey::ServerHost, "   ").unwrap_err();

        assert!(error.to_string().contains("must not be empty"));
        assert_eq!(fs::read_to_string(&path).unwrap(), original);
    }

    #[cfg(unix)]
    #[test]
    fn update_file_preserves_existing_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("config.toml");
        fs::write(&path, "[server]\nport = 3000\n").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();

        update_file(&path, ConfigKey::ServerPort, "4100").unwrap();

        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o640
        );
    }
}
