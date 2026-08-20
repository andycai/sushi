use std::process::Command;

#[test]
fn root_help_preserves_current_builtin_command_set() {
    let temp = tempfile::tempdir().expect("create temp dir");
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let config_path = temp.path().join("config.toml");
    let database_path = temp.path().join("data/sushi.db");
    std::fs::write(
        &config_path,
        format!(
            r#"
[database]
path = "{}"

[plugins]
directory = "{}"

[web]
templates_dir = "{}"
static_dir = "{}"

[runtime]
profiles_dir = "{}"
bundles_dir = "{}"
"#,
            database_path.display(),
            workspace.join("plugins").display(),
            workspace.join("web/templates").display(),
            workspace.join("web/static").display(),
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
            "--help",
        ])
        .output()
        .expect("run sushi --help");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("help output is utf-8");
    for command in ["serve", "plugin", "config", "seed", "inspect", "doctor"] {
        assert!(
            stdout
                .lines()
                .any(|line| line.trim_start().starts_with(command)),
            "missing root command {command} in help output"
        );
    }
    assert!(stdout.contains("--config <CONFIG>"));
    assert!(stdout.contains("--profile <PROFILE>"));
    assert!(database_path.exists());
}

#[test]
fn profile_without_host_cli_does_not_expose_builtin_business_commands() {
    let temp = tempfile::tempdir().expect("create temp dir");
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let profiles_dir = temp.path().join("profiles");
    let bundles_dir = temp.path().join("bundles");
    std::fs::create_dir_all(&profiles_dir).unwrap();
    std::fs::create_dir_all(&bundles_dir).unwrap();
    std::fs::write(
        bundles_dir.join("core.toml"),
        r#"
schema_version = 1
name = "core"

[[entries]]
id = "host.core"
source = "builtin:host-core"
enabled = true
required = true

[[entries]]
id = "policy.core"
source = "builtin:policy"
enabled = true
required = true
"#,
    )
    .unwrap();
    std::fs::write(
        profiles_dir.join("no-cli.toml"),
        "schema_version = 1\nname = \"no-cli\"\nbundles = [\"core\"]\n",
    )
    .unwrap();
    let config_path = temp.path().join("config.toml");
    std::fs::write(
        &config_path,
        format!(
            "[database]\npath = \"{}\"\n[plugins]\ndirectory = \"{}\"\n[web]\ntemplates_dir = \"{}\"\nstatic_dir = \"{}\"\n[runtime]\nprofiles_dir = \"{}\"\nbundles_dir = \"{}\"\n",
            temp.path().join("data/sushi.db").display(),
            workspace.join("plugins").display(),
            workspace.join("web/templates").display(),
            workspace.join("web/static").display(),
            profiles_dir.display(),
            bundles_dir.display(),
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_sushi"))
        .args([
            "--config",
            config_path.to_str().unwrap(),
            "--profile",
            "no-cli",
            "--help",
        ])
        .output()
        .expect("run profile-scoped help");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    for command in ["serve", "plugin", "config", "seed"] {
        assert!(
            !stdout
                .lines()
                .any(|line| line.trim_start().starts_with(command)),
            "unexpected command {command} in help output:\n{stdout}"
        );
    }
    for recovery_command in ["inspect", "doctor"] {
        assert!(
            stdout
                .lines()
                .any(|line| line.trim_start().starts_with(recovery_command)),
            "missing recovery command {recovery_command} in help output:\n{stdout}"
        );
    }
}

#[test]
fn config_get_reads_the_selected_config_file() {
    let temp = tempfile::tempdir().expect("create temp dir");
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let config_path = temp.path().join("selected.toml");
    std::fs::write(
        &config_path,
        format!(
            r#"
[server]
port = 4312

[database]
path = "{}"

[plugins]
directory = "{}"

[web]
templates_dir = "{}"
static_dir = "{}"

[runtime]
profiles_dir = "{}"
bundles_dir = "{}"
"#,
            temp.path().join("data/sushi.db").display(),
            workspace.join("plugins").display(),
            workspace.join("web/templates").display(),
            workspace.join("web/static").display(),
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
            "config",
            "get",
            "server.port",
        ])
        .output()
        .expect("run config get");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.lines().last(), Some("4312"));

    let default_output = run_config_command(&config_path, ["get", "server.body_size_limit"]);
    assert!(default_output.status.success());
    let default_stdout = String::from_utf8(default_output.stdout).unwrap();
    assert_eq!(default_stdout.lines().last(), Some("65536"));
}

#[test]
fn config_set_updates_only_the_target_key() {
    let temp = tempfile::tempdir().expect("create temp dir");
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let config_path = temp.path().join("selected.toml");
    std::fs::write(
        &config_path,
        format!(
            r#"
[server]
# Keep this operator note.
port = 4312

[database]
path = "{}"

[plugins]
directory = "{}"

[web]
templates_dir = "{}"
static_dir = "{}"

[runtime]
profiles_dir = "{}"
bundles_dir = "{}"

[custom]
enabled = true
"#,
            temp.path().join("data/sushi.db").display(),
            workspace.join("plugins").display(),
            workspace.join("web/templates").display(),
            workspace.join("web/static").display(),
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
            "config",
            "set",
            "server.port",
            "4410",
        ])
        .output()
        .expect("run config set");

    assert!(output.status.success());
    let updated = std::fs::read_to_string(&config_path).expect("read updated config");
    assert!(updated.contains("# Keep this operator note."));
    assert!(updated.contains("port = 4410"));
    assert!(updated.contains("[custom]\nenabled = true"));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.lines().last().unwrap().contains("restart"));
}

#[test]
fn config_commands_fail_closed_for_sensitive_unknown_and_invalid_values() {
    let temp = tempfile::tempdir().expect("create temp dir");
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let config_path = temp.path().join("selected.toml");
    let original = format!(
        r#"
[server]
port = 4312

[database]
path = "{}"

[jwt]
secret = "do-not-print-this-secret"

[plugins]
directory = "{}"

[web]
templates_dir = "{}"
static_dir = "{}"

[runtime]
profiles_dir = "{}"
bundles_dir = "{}"
"#,
        temp.path().join("data/sushi.db").display(),
        workspace.join("plugins").display(),
        workspace.join("web/templates").display(),
        workspace.join("web/static").display(),
        workspace.join("profiles").display(),
        workspace.join("bundles").display(),
    );
    std::fs::write(&config_path, &original).expect("write config");

    let sensitive = run_config_command(&config_path, ["get", "jwt.secret"]);
    assert!(!sensitive.status.success());
    let sensitive_output = format!(
        "{}{}",
        String::from_utf8_lossy(&sensitive.stdout),
        String::from_utf8_lossy(&sensitive.stderr)
    );
    assert!(sensitive_output.contains("sensitive"));
    assert!(!sensitive_output.contains("do-not-print-this-secret"));

    let secret_update =
        run_config_command(&config_path, ["set", "jwt.secret", "must-not-be-persisted"]);
    assert!(!secret_update.status.success());
    assert_eq!(std::fs::read_to_string(&config_path).unwrap(), original);

    for args in [
        ["set", "server.port", "70000"],
        ["set", "server.unknown", "1"],
    ] {
        let output = run_config_command(&config_path, args);
        assert!(!output.status.success());
        assert_eq!(std::fs::read_to_string(&config_path).unwrap(), original);
    }
}

#[test]
fn config_set_null_removes_runtime_profile() {
    let temp = tempfile::tempdir().expect("create temp dir");
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let config_path = temp.path().join("selected.toml");
    std::fs::write(
        &config_path,
        format!(
            r#"
[database]
path = "{}"

[plugins]
directory = "{}"

[web]
templates_dir = "{}"
static_dir = "{}"

[runtime]
profile = "minimal"
profiles_dir = "{}"
bundles_dir = "{}"
"#,
            temp.path().join("data/sushi.db").display(),
            workspace.join("plugins").display(),
            workspace.join("web/templates").display(),
            workspace.join("web/static").display(),
            workspace.join("profiles").display(),
            workspace.join("bundles").display(),
        ),
    )
    .expect("write config");

    let output = run_config_command(&config_path, ["set", "runtime.profile", "null"]);
    assert!(output.status.success());
    let updated = std::fs::read_to_string(&config_path).unwrap();
    assert!(!updated.contains("profile ="));
    assert!(updated.contains("profiles_dir ="));
}

fn run_config_command<const N: usize>(
    config_path: &std::path::Path,
    command_args: [&str; N],
) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_sushi"));
    command.args([
        "--config",
        config_path.to_str().unwrap(),
        "--profile",
        "minimal",
        "config",
    ]);
    command
        .args(command_args)
        .output()
        .expect("run config command")
}

#[test]
fn version_is_bootstrap_safe_when_configuration_is_missing() {
    let temp = tempfile::tempdir().expect("create temp dir");
    let missing_config = temp.path().join("missing.toml");
    let output = Command::new(env!("CARGO_BIN_EXE_sushi"))
        .args(["--config", missing_config.to_str().unwrap(), "--version"])
        .output()
        .expect("run bootstrap-safe version");

    assert!(output.status.success());
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .starts_with("sushi "));
    assert!(!temp.path().join("data/sushi.db").exists());
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

#[test]
fn inspect_profile_applies_cli_overlay_files_in_order() {
    let temp = tempfile::tempdir().expect("create temp dir");
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let config_path = temp.path().join("config.toml");
    std::fs::write(
        &config_path,
        format!(
            "[plugins]\ndirectory = \"{}\"\n[runtime]\nprofiles_dir = \"{}\"\nbundles_dir = \"{}\"\n",
            workspace.join("plugins").display(),
            workspace.join("profiles").display(),
            workspace.join("bundles").display(),
        ),
    )
    .unwrap();
    let overlay_path = temp.path().join("minimal-overlay.toml");
    std::fs::write(
        &overlay_path,
        r#"
schema_version = 1

[[overlays]]
id = "host.cli"
source = "builtin:host-cli"
enabled = true
required = true

[overlays.config]
mode = "temporary"
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_sushi"))
        .args([
            "--config",
            config_path.to_str().unwrap(),
            "--profile",
            "minimal",
            "--overlay-file",
            overlay_path.to_str().unwrap(),
            "inspect",
            "profile",
        ])
        .output()
        .expect("run inspect profile with overlay");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"mode\": \"temporary\""));
    assert!(stdout.contains(&format!("cli-overlay:{}", overlay_path.display())));
}

#[test]
fn serve_rejects_removed_surface_compatibility_flags() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");

    for flag in ["--api-only", "--admin-only"] {
        let output = Command::new(env!("CARGO_BIN_EXE_sushi"))
            .current_dir(&workspace)
            .args(["serve", flag])
            .output()
            .expect("run sushi serve with removed flag");

        assert!(!output.status.success(), "removed flag {flag} was accepted");
        let stderr = String::from_utf8(output.stderr).expect("help error output is utf-8");
        assert!(
            stderr.contains("unexpected argument") && stderr.contains(flag),
            "unexpected error for {flag}: {stderr}"
        );
    }
}

#[test]
fn inspect_capabilities_reports_owner_and_registration_source() {
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

[web]
templates_dir = "{}"
static_dir = "{}"

[runtime]
profiles_dir = "{}"
bundles_dir = "{}"
"#,
            database_path.display(),
            workspace.join("plugins").display(),
            workspace.join("web/templates").display(),
            workspace.join("web/static").display(),
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
            "api",
            "inspect",
            "capabilities",
        ])
        .output()
        .expect("run inspect capabilities");

    assert!(
        output.status.success(),
        "inspect capabilities failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("utf-8 inspect output");
    assert!(stdout.contains("\n[capabilities]\n"));
    assert!(stdout
        .lines()
        .any(|line| line.contains("owner=identity.core\tsource=builtin")));
    assert!(stdout
        .lines()
        .any(|line| line.contains("owner=cms.default\tsource=lua")));
}

#[cfg(unix)]
#[test]
fn serve_handles_sigterm_with_a_successful_graceful_exit() {
    use std::io::Read;
    use std::net::{TcpListener, TcpStream};
    use std::process::Stdio;
    use std::thread;
    use std::time::{Duration, Instant};

    let temp = tempfile::tempdir().expect("create temp dir");
    let config_path = temp.path().join("config.toml");
    let database_path = temp.path().join("data/sushi.db");
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let plugin_dir = temp.path().join("plugins/third_party/activation-probe");
    let profiles_dir = temp.path().join("profiles");
    let bundles_dir = temp.path().join("bundles");
    std::fs::create_dir_all(&plugin_dir).expect("create activation probe plugin");
    std::fs::create_dir_all(&profiles_dir).expect("create profiles dir");
    std::fs::create_dir_all(&bundles_dir).expect("create bundles dir");
    std::fs::write(
        plugin_dir.join("plugin.toml"),
        r#"
schema_version = 1

[plugin]
name = "activation-probe"
version = "0.1.0"
entry = "init.lua"
"#,
    )
    .expect("write activation probe manifest");
    std::fs::write(
        plugin_dir.join("init.lua"),
        r#"
function app.init()
    app.log.info("activation-probe-once")
end
"#,
    )
    .expect("write activation probe entrypoint");
    std::fs::write(
        bundles_dir.join("test.toml"),
        r#"
schema_version = 1
name = "test"

[[entries]]
id = "host.core"
source = "builtin:host-core"
enabled = true
required = true

[[entries]]
id = "host.cli"
source = "builtin:host-cli"
enabled = true
required = true

[[entries]]
id = "policy.core"
source = "builtin:policy"
enabled = true
required = true

[[entries]]
id = "probe.activation"
source = "lua:third_party/activation-probe"
enabled = true
required = false

[entries.grants]
approved = true
"#,
    )
    .expect("write activation probe bundle");
    std::fs::write(
        profiles_dir.join("default.toml"),
        "schema_version = 1\nname = \"default\"\nbundles = [\"test\"]\n",
    )
    .expect("write activation probe profile");
    let port = TcpListener::bind(("127.0.0.1", 0))
        .expect("reserve local port")
        .local_addr()
        .expect("read local address")
        .port();
    std::fs::write(
        &config_path,
        format!(
            r#"
[database]
path = "{}"

[plugins]
directory = "{}"

[web]
templates_dir = "{}"
static_dir = "{}"

[runtime]
profiles_dir = "{}"
bundles_dir = "{}"
"#,
            database_path.display(),
            temp.path().join("plugins").display(),
            workspace.join("web/templates").display(),
            workspace.join("web/static").display(),
            profiles_dir.display(),
            bundles_dir.display(),
        ),
    )
    .expect("write config");

    let mut child = Command::new(env!("CARGO_BIN_EXE_sushi"))
        .current_dir(&workspace)
        .args([
            "--config",
            config_path.to_str().unwrap(),
            "--profile",
            "default",
            "serve",
            "--host",
            "127.0.0.1",
            "--port",
            &port.to_string(),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start sushi server");

    let ready_deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            break;
        }
        if let Some(status) = child.try_wait().expect("poll sushi server") {
            panic!("sushi server exited before accepting connections: {status}");
        }
        if Instant::now() >= ready_deadline {
            child.kill().ok();
            child.wait().ok();
            panic!("sushi server did not become ready");
        }
        thread::sleep(Duration::from_millis(20));
    }

    let signal = Command::new("kill")
        .args(["-TERM", &child.id().to_string()])
        .status()
        .expect("send SIGTERM");
    assert!(signal.success(), "failed to send SIGTERM");

    let exit_deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(status) = child.try_wait().expect("poll graceful shutdown") {
            break status;
        }
        if Instant::now() >= exit_deadline {
            child.kill().ok();
            child.wait().ok();
            panic!("sushi server did not exit after SIGTERM");
        }
        thread::sleep(Duration::from_millis(20));
    };
    assert!(status.success(), "SIGTERM shutdown failed: {status}");

    let mut stdout = String::new();
    child
        .stdout
        .take()
        .expect("capture sushi stdout")
        .read_to_string(&mut stdout)
        .expect("read sushi stdout");
    let mut stderr = String::new();
    child
        .stderr
        .take()
        .expect("capture sushi stderr")
        .read_to_string(&mut stderr)
        .expect("read sushi stderr");
    assert_eq!(
        stdout.matches("activation-probe-once").count()
            + stderr.matches("activation-probe-once").count(),
        1,
        "serve must activate each runtime entry exactly once; stdout:\n{stdout}\nstderr:\n{stderr}"
    );
}

#[test]
fn doctor_reports_required_approval_source_and_repair() {
    let temp = tempfile::tempdir().expect("create temp dir");
    let plugins_dir = temp.path().join("plugins/third_party/required-probe");
    let profiles_dir = temp.path().join("profiles");
    let bundles_dir = temp.path().join("bundles");
    std::fs::create_dir_all(&plugins_dir).unwrap();
    std::fs::create_dir_all(&profiles_dir).unwrap();
    std::fs::create_dir_all(&bundles_dir).unwrap();
    std::fs::write(
        plugins_dir.join("plugin.toml"),
        r#"
schema_version = 1
[plugin]
name = "required-probe"
version = "0.1.0"
entry = "init.lua"
[permissions]
routes = true
"#,
    )
    .unwrap();
    std::fs::write(
        plugins_dir.join("init.lua"),
        "error('doctor must not execute plugin code')",
    )
    .unwrap();
    std::fs::write(
        bundles_dir.join("test.toml"),
        r#"
schema_version = 1
name = "test"
[[entries]]
id = "probe.required"
source = "lua:third_party/required-probe"
enabled = true
required = true
"#,
    )
    .unwrap();
    std::fs::write(
        profiles_dir.join("default.toml"),
        "schema_version = 1\nname = \"default\"\nbundles = [\"test\"]\n",
    )
    .unwrap();
    let config_path = temp.path().join("config.toml");
    std::fs::write(
        &config_path,
        format!(
            "[database]\npath = \"{}\"\n[plugins]\ndirectory = \"{}\"\n[runtime]\nprofiles_dir = \"{}\"\nbundles_dir = \"{}\"\n",
            temp.path().join("missing.db").display(),
            temp.path().join("plugins").display(),
            profiles_dir.display(),
            bundles_dir.display()
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_sushi"))
        .args(["--config", config_path.to_str().unwrap(), "doctor"])
        .output()
        .expect("run doctor");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("entry 'probe.required' source 'lua:third_party/required-probe'"));
    assert!(stderr.contains("set grants.approved = true"));
    assert!(!temp.path().join("missing.db").exists());
}

#[test]
fn doctor_reports_forward_recovery_for_partial_required_migration() {
    let temp = tempfile::tempdir().expect("create temp dir");
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let database_path = temp.path().join("partial.db");
    {
        let connection = rusqlite::Connection::open(&database_path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE plugin_state (name TEXT PRIMARY KEY, enabled INTEGER NOT NULL DEFAULT 1, loaded INTEGER NOT NULL DEFAULT 0, version TEXT, loaded_at TEXT, plugin_id TEXT NOT NULL DEFAULT '');",
            )
            .unwrap();
    }
    let config_path = temp.path().join("config.toml");
    std::fs::write(
        &config_path,
        format!(
            "[database]\npath = \"{}\"\n[plugins]\ndirectory = \"{}\"\n[runtime]\nprofiles_dir = \"{}\"\nbundles_dir = \"{}\"\n",
            database_path.display(),
            workspace.join("plugins").display(),
            workspace.join("profiles").display(),
            workspace.join("bundles").display()
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_sushi"))
        .args([
            "--config",
            config_path.to_str().unwrap(),
            "--profile",
            "minimal",
            "doctor",
        ])
        .output()
        .expect("run doctor");
    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stdout.contains(
        "migration:builtin/host-core:008_plugin_governance_v1\tstatus=recovery-required"
    ));
    assert!(stderr.contains("back up the database"));
    assert!(stderr.contains("forward-only recovery"));
}
