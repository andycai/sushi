use sushi_core::lua::contract::{ContractSchemaVersion, LuaCapabilityContract};

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
