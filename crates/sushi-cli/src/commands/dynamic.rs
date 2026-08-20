use anyhow::{Context, Result};
use clap::Command;
use std::ffi::OsString;
use std::path::PathBuf;

use crate::commands::authorization::ensure_command_authorized;
use sushi_core::context::SushiContext;
use sushi_core::runtime::CapabilitySnapshot;

#[derive(Debug, Clone)]
struct BootstrapOptions {
    role: String,
    config: PathBuf,
    profile: Option<String>,
    overlay_paths: Vec<PathBuf>,
    version: bool,
    command: Option<String>,
    command_args: Vec<String>,
}

pub fn command(snapshot: &CapabilitySnapshot) -> Command {
    let mut root = Command::new("sushi")
        .version(env!("CARGO_PKG_VERSION"))
        .about("A modular application platform")
        .arg(
            clap::Arg::new("role")
                .long("role")
                .global(true)
                .value_name("ROLE")
                .help("CLI principal role used for command authorization"),
        )
        .arg(
            clap::Arg::new("config")
                .short('c')
                .long("config")
                .global(true)
                .value_name("CONFIG")
                .default_value("config.toml")
                .help("Config file used for bootstrap and profile resolution"),
        )
        .arg(
            clap::Arg::new("profile")
                .long("profile")
                .global(true)
                .value_name("PROFILE")
                .help("Runtime profile override"),
        )
        .arg(
            clap::Arg::new("overlay-file")
                .long("overlay-file")
                .global(true)
                .value_name("PATH")
                .action(clap::ArgAction::Append)
                .help("Temporary runtime entry overlay file"),
        )
        .subcommand(
            Command::new("inspect")
                .about("Inspect resolved runtime state")
                .arg(
                    clap::Arg::new("args")
                        .value_name("ARGS")
                        .num_args(0..)
                        .trailing_var_arg(true)
                        .allow_hyphen_values(true),
                ),
        )
        .subcommand(
            Command::new("doctor")
                .about("Check profile resolution without starting product surfaces"),
        );

    for registration in snapshot.cli_commands() {
        let spec = &registration.value;
        if matches!(spec.name.as_str(), "inspect" | "doctor") {
            continue;
        }
        let policy = spec
            .policy_key
            .as_deref()
            .map(|value| format!(", policy={value}"))
            .unwrap_or_default();
        let about = format!(
            "{} [owner={}, source={}{}]",
            spec.description, registration.owner, registration.source, policy
        );
        root = root.subcommand(
            Command::new(spec.name.clone()).about(about).arg(
                clap::Arg::new("args")
                    .value_name("ARGS")
                    .num_args(0..)
                    .trailing_var_arg(true)
                    .allow_hyphen_values(true),
            ),
        );
    }

    root
}

pub async fn run(argv: Vec<OsString>) -> Result<()> {
    let bootstrap = parse_bootstrap_options(&argv)?;

    if bootstrap.version {
        println!("sushi {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    if bootstrap.command.as_deref() == Some("doctor") {
        if !bootstrap.command_args.is_empty() {
            anyhow::bail!("doctor does not accept positional arguments")
        }
        return crate::commands::doctor::run_with_overlays(
            &bootstrap.config,
            bootstrap.profile.as_deref(),
            &bootstrap.overlay_paths,
        )
        .await;
    }
    if bootstrap.command.as_deref() == Some("inspect")
        && bootstrap.command_args.first().map(String::as_str) == Some("profile")
    {
        return run_inspect_recovery(&bootstrap).await;
    }

    let ctx = crate::app::bootstrap_with_options(
        Some(&bootstrap.config),
        bootstrap.profile.as_deref(),
        &bootstrap.overlay_paths,
        &bootstrap.role,
    )
    .await
    .context("failed to bootstrap runtime for dynamic CLI command")?;
    let result = run_with_context(&argv, &bootstrap, &ctx).await;
    ctx.shutdown().await;
    result
}

async fn run_with_context(
    argv: &[OsString],
    bootstrap: &BootstrapOptions,
    ctx: &SushiContext,
) -> Result<()> {
    let snapshot = ctx.plugins.capability_snapshot().await;
    let matches = match command(&snapshot).try_get_matches_from(normalize_runtime_argv(argv)) {
        Ok(matches) => matches,
        Err(error)
            if matches!(
                error.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) =>
        {
            print!("{error}");
            return Ok(());
        }
        Err(error) => return Err(anyhow::anyhow!(error.to_string())),
    };
    let Some((command_name, command_matches)) = matches.subcommand() else {
        anyhow::bail!("a command is required")
    };
    let args = command_matches
        .get_many::<String>("args")
        .map(|values| values.cloned().collect::<Vec<_>>())
        .unwrap_or_default();

    let authorization_target = command_authorization_target(command_name, &args);
    ensure_command_authorized(ctx, &bootstrap.role, &authorization_target).await?;
    match ctx.plugins.call_cli_handler(command_name, &args).await {
        Some(Ok(output)) => {
            if !output.is_empty() {
                println!("{output}");
            }
        }
        Some(Err(error)) => anyhow::bail!("plugin error: {error}"),
        None => anyhow::bail!("command '{}' not found in runtime snapshot", command_name),
    }
    Ok(())
}

fn command_authorization_target(command_name: &str, args: &[String]) -> String {
    match command_name {
        "plugin" => match args.first().map(String::as_str) {
            Some("list") => "plugin:list".to_string(),
            Some("status") => "plugin:status".to_string(),
            Some("enable") => "plugin:enable".to_string(),
            Some("disable") => "plugin:disable".to_string(),
            _ => command_name.to_string(),
        },
        "config" => match args.first().map(String::as_str) {
            Some("get") => "config:get".to_string(),
            Some("set") => "config:set".to_string(),
            _ => command_name.to_string(),
        },
        _ => command_name.to_string(),
    }
}

async fn run_inspect_recovery(options: &BootstrapOptions) -> Result<()> {
    let args = crate::builtin::parse_args::<crate::commands::inspect::InspectArgs>(
        "inspect",
        &options.command_args,
    )
    .map_err(anyhow::Error::msg)?;
    crate::commands::inspect::run_with_overlays(
        args,
        &options.config,
        options.profile.as_deref(),
        &options.overlay_paths,
    )
    .await
}

fn parse_bootstrap_options(argv: &[OsString]) -> Result<BootstrapOptions> {
    let mut role = std::env::var("SUSHI_CLI_ROLE").unwrap_or_else(|_| "admin".to_string());
    let mut config = PathBuf::from("config.toml");
    let mut profile = None;
    let mut overlay_paths = Vec::new();
    let mut version = false;
    let mut command = None;
    let mut command_args = Vec::new();
    let mut index = 1;
    let mut after_separator = false;

    while index < argv.len() {
        let value = argv[index].to_string_lossy().to_string();
        if !after_separator {
            if value == "--" {
                after_separator = true;
                index += 1;
                continue;
            }
            if let Some(next) = argv.get(index + 1) {
                if value == "--role" {
                    role = next.to_string_lossy().to_string();
                    index += 2;
                    continue;
                }
                if value == "--config" || value == "-c" {
                    config = PathBuf::from(next);
                    index += 2;
                    continue;
                }
                if value == "--profile" {
                    profile = Some(next.to_string_lossy().to_string());
                    index += 2;
                    continue;
                }
                if value == "--overlay-file" {
                    overlay_paths.push(PathBuf::from(next));
                    index += 2;
                    continue;
                }
            }
            if let Some(value) = value.strip_prefix("--role=") {
                role = value.to_string();
                index += 1;
                continue;
            }
            if let Some(value) = value.strip_prefix("--config=") {
                config = PathBuf::from(value);
                index += 1;
                continue;
            }
            if let Some(value) = value.strip_prefix("--profile=") {
                profile = Some(value.to_string());
                index += 1;
                continue;
            }
            if let Some(value) = value.strip_prefix("--overlay-file=") {
                overlay_paths.push(PathBuf::from(value));
                index += 1;
                continue;
            }
            if value == "--version" || value == "-V" {
                version = true;
                index += 1;
                continue;
            }
            if value == "--help" || value == "-h" {
                index += 1;
                continue;
            }
        }

        if command.is_none() {
            command = Some(value);
        } else {
            command_args.push(value);
        }
        index += 1;
    }

    role = crate::commands::authorization::resolve_cli_role(Some(&role), None);
    Ok(BootstrapOptions {
        role,
        config,
        profile,
        overlay_paths,
        version,
        command,
        command_args,
    })
}

fn normalize_runtime_argv(argv: &[OsString]) -> Vec<OsString> {
    let mut normalized = Vec::with_capacity(argv.len());
    if let Some(program) = argv.first() {
        normalized.push(program.clone());
    }
    let mut index = 1;
    while index < argv.len() {
        let value = argv[index].to_string_lossy();
        if value == "--role"
            || value == "--config"
            || value == "-c"
            || value == "--profile"
            || value == "--overlay-file"
        {
            index += 2;
            continue;
        }
        if value.starts_with("--role=")
            || value.starts_with("--config=")
            || value.starts_with("--profile=")
            || value.starts_with("--overlay-file=")
        {
            index += 1;
            continue;
        }
        normalized.push(argv[index].clone());
        index += 1;
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::path::Path;
    use sushi_core::runtime::{CliCommandSpec, PluginInstanceId, RegistrationSource};

    async fn snapshot_with_commands() -> CapabilitySnapshot {
        let registry = sushi_core::runtime::CapabilityRegistry::new();
        let owner = PluginInstanceId::legacy("test-plugin");
        let mut staged = registry.stage_with_source(owner, RegistrationSource::Lua);
        staged.register_cli(CliCommandSpec::new(
            "notes-list",
            "List notes",
            "notes",
            "handler::list",
        ));
        registry.commit(staged).await.unwrap();
        registry.snapshot().await.as_ref().clone()
    }

    #[tokio::test]
    async fn dynamic_command_tree_exposes_registered_commands_and_trailing_args() {
        let snapshot = snapshot_with_commands().await;
        let matches = command(&snapshot)
            .try_get_matches_from(["sushi", "--role", "editor", "notes-list", "--limit", "10"])
            .unwrap();
        assert_eq!(
            matches.get_one::<String>("role").map(String::as_str),
            Some("editor")
        );
        let (_, subcommand) = matches.subcommand().unwrap();
        assert_eq!(
            subcommand
                .get_many::<String>("args")
                .unwrap()
                .cloned()
                .collect::<Vec<_>>(),
            vec!["--limit", "10"]
        );
    }

    #[test]
    fn bootstrap_parser_extracts_globals_and_external_command() {
        let options = parse_bootstrap_options(&[
            OsString::from("sushi"),
            OsString::from("--config"),
            OsString::from("custom.toml"),
            OsString::from("plugin"),
            OsString::from("list"),
        ])
        .unwrap();
        assert_eq!(options.config, Path::new("custom.toml"));
        assert_eq!(options.command.as_deref(), Some("plugin"));
        assert_eq!(options.command_args, vec!["list"]);
    }

    #[test]
    fn runtime_argv_preserves_command_flags() {
        let argv = vec![
            OsString::from("sushi"),
            OsString::from("--config"),
            OsString::from("custom.toml"),
            OsString::from("plugin"),
            OsString::from("disable"),
            OsString::from("kv-store"),
            OsString::from("--reason"),
            OsString::from("maintenance"),
        ];
        assert_eq!(
            normalize_runtime_argv(&argv),
            vec![
                OsString::from("sushi"),
                OsString::from("plugin"),
                OsString::from("disable"),
                OsString::from("kv-store"),
                OsString::from("--reason"),
                OsString::from("maintenance"),
            ]
        );
    }

    #[test]
    fn authorization_target_distinguishes_config_reads_and_writes() {
        assert_eq!(
            command_authorization_target("config", &["get".to_string(), "server.port".to_string()]),
            "config:get"
        );
        assert_eq!(
            command_authorization_target(
                "config",
                &[
                    "set".to_string(),
                    "server.port".to_string(),
                    "4100".to_string()
                ]
            ),
            "config:set"
        );
    }
}
