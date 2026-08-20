use serde_json::json;
use std::fs;
use std::path::Path;
use sushi_core::runtime::{ProfileError, RuntimePluginSource, RuntimeProfileResolver};
use tempfile::TempDir;

fn create_plugin(root: &Path, tier: &str, name: &str) {
    let plugin_dir = root.join(tier).join(name);
    fs::create_dir_all(&plugin_dir).unwrap();
    fs::write(
        plugin_dir.join("plugin.toml"),
        "[plugin]\nname = \"probe\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
}

fn resolver(temp: &TempDir) -> RuntimeProfileResolver {
    RuntimeProfileResolver::new(
        temp.path().join("profiles"),
        temp.path().join("bundles"),
        temp.path().join("plugins"),
    )
}

#[test]
fn overlay_replaces_the_complete_entry_and_tracks_origin() {
    let temp = TempDir::new().unwrap();
    create_plugin(&temp.path().join("plugins"), "official", "probe");
    fs::create_dir_all(temp.path().join("profiles")).unwrap();
    fs::create_dir_all(temp.path().join("bundles")).unwrap();
    fs::write(
        temp.path().join("bundles/base.toml"),
        r#"
schema_version = 1
name = "base"

[[entries]]
id = "probe.default"
source = "lua:official/probe"
enabled = true
required = false

[entries.config]
retained_only_by_deep_merge = true
mode = "bundle"
"#,
    )
    .unwrap();
    fs::write(
        temp.path().join("profiles/default.toml"),
        r#"
schema_version = 1
name = "default"
bundles = ["base"]

[[overlays]]
id = "probe.default"
source = "lua:official/probe"
enabled = true
required = false

[overlays.config]
mode = "profile"
"#,
    )
    .unwrap();

    let profile = resolver(&temp).resolve("default").unwrap();
    let entry = &profile.entries()[0];
    assert_eq!(entry.id.as_str(), "probe.default");
    assert_eq!(entry.config, json!({"mode": "profile"}));
    assert_eq!(entry.origin, "profile:default");
    assert_eq!(
        entry.source.resolved_path(),
        Some(temp.path().join("plugins/official/probe").as_path())
    );
}

#[test]
fn cli_overlay_files_apply_in_order_and_track_the_last_source() {
    let temp = TempDir::new().unwrap();
    fs::create_dir_all(temp.path().join("profiles")).unwrap();
    fs::create_dir_all(temp.path().join("bundles")).unwrap();
    fs::create_dir_all(temp.path().join("plugins")).unwrap();
    fs::write(
        temp.path().join("bundles/base.toml"),
        r#"
schema_version = 1
name = "base"

[[entries]]
id = "host.core"
source = "builtin:host-core"
enabled = true
required = true
"#,
    )
    .unwrap();
    fs::write(
        temp.path().join("profiles/default.toml"),
        "schema_version = 1\nname = \"default\"\nbundles = [\"base\"]\n",
    )
    .unwrap();
    let first = temp.path().join("first-overlay.toml");
    fs::write(
        &first,
        r#"
schema_version = 1

[[overlays]]
id = "host.core"
source = "builtin:host-core"
enabled = true
required = true

[overlays.config]
mode = "first"
"#,
    )
    .unwrap();
    let second = temp.path().join("second-overlay.toml");
    fs::write(
        &second,
        r#"
schema_version = 1

[[overlays]]
id = "host.core"
source = "builtin:host-core"
enabled = true
required = true

[overlays.config]
mode = "second"
"#,
    )
    .unwrap();

    let profile = resolver(&temp)
        .resolve_with_overlays("default", &[first, second.clone()])
        .unwrap();
    let entry = &profile.entries()[0];
    assert_eq!(entry.config, json!({"mode": "second"}));
    assert_eq!(entry.origin, format!("cli-overlay:{}", second.display()));
}

#[test]
fn duplicate_bundle_entry_ids_are_rejected() {
    let temp = TempDir::new().unwrap();
    fs::create_dir_all(temp.path().join("profiles")).unwrap();
    fs::create_dir_all(temp.path().join("bundles")).unwrap();
    fs::create_dir_all(temp.path().join("plugins")).unwrap();
    for bundle in ["first", "second"] {
        fs::write(
            temp.path().join(format!("bundles/{bundle}.toml")),
            format!(
                r#"
schema_version = 1
name = "{bundle}"

[[entries]]
id = "api.core"
source = "builtin:api-core"
enabled = true
required = true
"#
            ),
        )
        .unwrap();
    }
    fs::write(
        temp.path().join("profiles/default.toml"),
        "schema_version = 1\nname = \"default\"\nbundles = [\"first\", \"second\"]\n",
    )
    .unwrap();

    let error = resolver(&temp).resolve("default").unwrap_err();
    assert!(matches!(error, ProfileError::DuplicateEntryId { .. }));
}

#[test]
fn policy_builtin_can_be_composed_as_a_required_runtime_entry() {
    let temp = TempDir::new().unwrap();
    fs::create_dir_all(temp.path().join("profiles")).unwrap();
    fs::create_dir_all(temp.path().join("bundles")).unwrap();
    fs::create_dir_all(temp.path().join("plugins")).unwrap();
    fs::write(
        temp.path().join("bundles/base.toml"),
        r#"
schema_version = 1
name = "base"

[[entries]]
id = "policy.core"
source = "builtin:policy"
enabled = true
required = true
"#,
    )
    .unwrap();
    fs::write(
        temp.path().join("profiles/default.toml"),
        "schema_version = 1\nname = \"default\"\nbundles = [\"base\"]\n",
    )
    .unwrap();

    let profile = resolver(&temp).resolve("default").unwrap();
    let entry = &profile.entries()[0];
    assert_eq!(entry.id.as_str(), "policy.core");
    assert_eq!(entry.source.reference(), "builtin:policy");
    assert!(entry.enabled);
    assert!(entry.required);
}

#[test]
fn admin_shell_builtin_can_be_composed_as_a_required_runtime_entry() {
    let temp = TempDir::new().unwrap();
    fs::create_dir_all(temp.path().join("profiles")).unwrap();
    fs::create_dir_all(temp.path().join("bundles")).unwrap();
    fs::create_dir_all(temp.path().join("plugins")).unwrap();
    fs::write(
        temp.path().join("bundles/admin.toml"),
        r#"
schema_version = 1
name = "admin"

[[entries]]
id = "admin.shell"
source = "builtin:admin-shell"
enabled = true
required = true
"#,
    )
    .unwrap();
    fs::write(
        temp.path().join("profiles/default.toml"),
        "schema_version = 1\nname = \"default\"\nbundles = [\"admin\"]\n",
    )
    .unwrap();

    let profile = resolver(&temp).resolve("default").unwrap();
    let entry = &profile.entries()[0];
    assert_eq!(entry.id.as_str(), "admin.shell");
    assert_eq!(entry.source.reference(), "builtin:admin-shell");
    assert!(entry.enabled);
    assert!(entry.required);
}

#[test]
fn admin_shell_profile_entry_cannot_be_disabled() {
    let temp = TempDir::new().unwrap();
    fs::create_dir_all(temp.path().join("profiles")).unwrap();
    fs::create_dir_all(temp.path().join("bundles")).unwrap();
    fs::create_dir_all(temp.path().join("plugins")).unwrap();
    fs::write(
        temp.path().join("bundles/admin.toml"),
        r#"
schema_version = 1
name = "admin"

[[entries]]
id = "admin.shell"
source = "builtin:admin-shell"
enabled = true
required = true
"#,
    )
    .unwrap();
    fs::write(
        temp.path().join("profiles/default.toml"),
        r#"
schema_version = 1
name = "default"
bundles = ["admin"]

[[overlays]]
id = "admin.shell"
source = "builtin:admin-shell"
enabled = false
required = true
"#,
    )
    .unwrap();

    let error = resolver(&temp).resolve("default").unwrap_err();
    assert!(matches!(error, ProfileError::RequiredEntryDisabled { .. }));
}

#[test]
fn governance_builtin_can_be_composed_as_a_required_runtime_entry() {
    let temp = TempDir::new().unwrap();
    fs::create_dir_all(temp.path().join("profiles")).unwrap();
    fs::create_dir_all(temp.path().join("bundles")).unwrap();
    fs::create_dir_all(temp.path().join("plugins")).unwrap();
    fs::write(
        temp.path().join("bundles/admin.toml"),
        r#"
schema_version = 1
name = "admin"

[[entries]]
id = "governance.admin"
source = "builtin:governance"
enabled = true
required = true
"#,
    )
    .unwrap();
    fs::write(
        temp.path().join("profiles/default.toml"),
        "schema_version = 1\nname = \"default\"\nbundles = [\"admin\"]\n",
    )
    .unwrap();

    let profile = resolver(&temp).resolve("default").unwrap();
    let entry = &profile.entries()[0];
    assert_eq!(entry.id.as_str(), "governance.admin");
    assert_eq!(entry.source.reference(), "builtin:governance");
    assert!(entry.enabled);
    assert!(entry.required);
}

#[test]
fn unknown_sources_and_builtin_factories_fail_closed() {
    let temp = TempDir::new().unwrap();
    fs::create_dir_all(temp.path().join("profiles")).unwrap();
    fs::create_dir_all(temp.path().join("bundles")).unwrap();
    fs::create_dir_all(temp.path().join("plugins")).unwrap();
    fs::write(
        temp.path().join("profiles/default.toml"),
        r#"
schema_version = 1
name = "default"
bundles = ["base"]

[[overlays]]
id = "host.core"
source = "remote:https://example.invalid/plugin"
enabled = true
required = false
"#,
    )
    .unwrap();
    fs::write(
        temp.path().join("bundles/base.toml"),
        r#"
schema_version = 1
name = "base"

[[entries]]
id = "host.core"
source = "builtin:host-core"
enabled = true
required = true
"#,
    )
    .unwrap();
    let error = resolver(&temp).resolve("default").unwrap_err();
    assert!(matches!(error, ProfileError::UnknownSource { .. }));

    fs::write(
        temp.path().join("profiles/default.toml"),
        r#"
schema_version = 1
name = "default"
bundles = ["base"]

[[overlays]]
id = "host.core"
source = "builtin:not-registered"
enabled = true
required = false
"#,
    )
    .unwrap();
    let error = resolver(&temp).resolve("default").unwrap_err();
    assert!(matches!(error, ProfileError::UnknownBuiltin { .. }));
}

#[test]
fn overlays_must_target_existing_entries_and_required_entries_must_be_enabled() {
    let temp = TempDir::new().unwrap();
    fs::create_dir_all(temp.path().join("profiles")).unwrap();
    fs::create_dir_all(temp.path().join("bundles")).unwrap();
    fs::create_dir_all(temp.path().join("plugins")).unwrap();
    fs::write(
        temp.path().join("profiles/default.toml"),
        r#"
schema_version = 1
name = "default"
bundles = []

[[overlays]]
id = "api.core"
source = "builtin:api-core"
enabled = false
required = true
"#,
    )
    .unwrap();
    let error = resolver(&temp).resolve("default").unwrap_err();
    assert!(matches!(error, ProfileError::UnknownOverlayTarget { .. }));

    fs::write(
        temp.path().join("bundles/base.toml"),
        r#"
schema_version = 1
name = "base"

[[entries]]
id = "api.core"
source = "builtin:api-core"
enabled = true
required = true
"#,
    )
    .unwrap();
    fs::write(
        temp.path().join("profiles/default.toml"),
        r#"
schema_version = 1
name = "default"
bundles = ["base"]

[[overlays]]
id = "api.core"
source = "builtin:api-core"
enabled = false
required = true
"#,
    )
    .unwrap();
    let error = resolver(&temp).resolve("default").unwrap_err();
    assert!(matches!(error, ProfileError::RequiredEntryDisabled { .. }));
}

#[test]
fn missing_implicit_default_fails_closed() {
    let temp = TempDir::new().unwrap();
    create_plugin(&temp.path().join("plugins"), "third_party", "zeta");
    create_plugin(&temp.path().join("plugins"), "official", "beta");
    create_plugin(&temp.path().join("plugins"), "official", "alpha");

    let error = resolver(&temp).resolve_configured(None).unwrap_err();
    assert!(matches!(error, ProfileError::ProfileNotFound { .. }));
}

#[test]
fn shipped_profiles_have_stable_portable_dumps() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let resolver = RuntimeProfileResolver::new(
        workspace.join("profiles"),
        workspace.join("bundles"),
        workspace.join("plugins"),
    );

    let expected = [
        ("default", include_str!("fixtures/profile/default.json")),
        ("api", include_str!("fixtures/profile/api.json")),
        ("admin", include_str!("fixtures/profile/admin.json")),
        ("minimal", include_str!("fixtures/profile/minimal.json")),
    ];

    for (name, expected_dump) in expected {
        let profile = resolver.resolve(name).unwrap();
        assert_eq!(profile.name(), name);
        let dump = profile.dump_json().unwrap();
        assert_eq!(dump, profile.dump_json().unwrap());
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&dump).unwrap(),
            serde_json::from_str::<serde_json::Value>(expected_dump).unwrap()
        );
        assert!(profile.entries().iter().all(|entry| match &entry.source {
            RuntimePluginSource::Builtin { .. } => entry.required,
            RuntimePluginSource::Lua { .. } => !entry.required,
        }));
    }
}
