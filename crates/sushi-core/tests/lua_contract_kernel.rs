use sushi_core::lua::contract::{ContractSchemaVersion, LuaCapabilityContract};
use sushi_core::lua::permission::engine::{CapabilityKind, PermissionDecisionEngine};
use sushi_core::plugin::{DatabasePermission, Permissions};

#[test]
fn contract_kernel_exports_v2_types() {
    let version = ContractSchemaVersion::V2;
    let contract = LuaCapabilityContract::default();
    assert_eq!(version.as_str(), "v2");
    assert!(contract.entries.is_empty());

    let tagged_api = serde_json::json!({
        "surface": "api",
        "method": "GET",
        "path": "/health"
    });
    let parsed: sushi_core::lua::contract::LuaCapabilityEntry =
        serde_json::from_value(tagged_api.clone()).expect("api variant should deserialize");
    assert_eq!(
        serde_json::to_value(&parsed).expect("api variant should serialize"),
        tagged_api
    );

    let roundtrip = LuaCapabilityContract {
        entries: vec![parsed],
    };
    let encoded = serde_json::to_string(&roundtrip).expect("contract should serialize");
    let decoded: LuaCapabilityContract =
        serde_json::from_str(&encoded).expect("contract should deserialize");
    assert_eq!(decoded, roundtrip);
}

#[test]
fn deny_by_default_hides_unauthorized_capabilities() {
    let engine = PermissionDecisionEngine::new(Permissions::default(), true);

    assert!(!engine.is_visible(CapabilityKind::ApiRoute));
    assert!(!engine.is_visible(CapabilityKind::AdminPage));
    assert!(!engine.is_visible(CapabilityKind::CliCommand));
    assert!(!engine.is_visible(CapabilityKind::WebRender));
    assert!(!engine.is_visible(CapabilityKind::DbRead));
    assert!(!engine.is_visible(CapabilityKind::DbWrite));
    assert!(engine.is_visible(CapabilityKind::Event));
    assert!(engine.is_visible(CapabilityKind::Fs));
}

#[test]
fn db_write_visibility_requires_write_or_admin() {
    let none_db = PermissionDecisionEngine::new(
        Permissions {
            database: DatabasePermission::None,
            ..Permissions::default()
        },
        true,
    );
    assert!(!none_db.is_visible(CapabilityKind::DbWrite));

    let read_only_db = PermissionDecisionEngine::new(
        Permissions {
            database: DatabasePermission::ReadOnly,
            ..Permissions::default()
        },
        true,
    );
    assert!(!read_only_db.is_visible(CapabilityKind::DbWrite));

    let write_db = PermissionDecisionEngine::new(
        Permissions {
            database: DatabasePermission::Write,
            ..Permissions::default()
        },
        true,
    );
    assert!(write_db.is_visible(CapabilityKind::DbWrite));

    let admin_db = PermissionDecisionEngine::new(
        Permissions {
            database: DatabasePermission::Admin,
            ..Permissions::default()
        },
        true,
    );
    assert!(admin_db.is_visible(CapabilityKind::DbWrite));
}
