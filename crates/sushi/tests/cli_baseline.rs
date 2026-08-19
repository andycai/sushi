use std::process::Command;

#[test]
fn root_help_preserves_current_builtin_command_set() {
    let output = Command::new(env!("CARGO_BIN_EXE_sushi"))
        .arg("--help")
        .output()
        .expect("run sushi --help");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("help output is utf-8");
    for command in ["serve", "run", "plugin", "config", "seed", "inspect"] {
        assert!(
            stdout
                .lines()
                .any(|line| line.trim_start().starts_with(command)),
            "missing root command {command} in help output"
        );
    }
    assert!(stdout.contains("--config <CONFIG>"));
    assert!(stdout.contains("--profile <PROFILE>"));
}

#[test]
fn inspect_profile_is_bootstrap_safe_and_uses_global_options() {
    let temp = tempfile::tempdir().expect("create temp dir");
    let config_path = temp.path().join("config.toml");
    let database_path = temp.path().join("data/sushi.db");
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::write(
        &config_path,
        format!(
            r#"
[database]
path = "{}"

[plugins]
directory = "{}"

[runtime]
profiles_dir = "{}"
bundles_dir = "{}"
"#,
            database_path.display(),
            workspace.join("plugins").display(),
            workspace.join("profiles").display(),
            workspace.join("bundles").display(),
        ),
    )
    .expect("write config");

    let output = Command::new(env!("CARGO_BIN_EXE_sushi"))
        .args([
            "--config",
            config_path.to_str().unwrap(),
            "--profile",
            "minimal",
            "inspect",
            "profile",
        ])
        .output()
        .expect("run inspect profile");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("utf-8 inspect output");
    assert!(stdout.contains("\"name\": \"minimal\""));
    assert!(!database_path.exists());
}
