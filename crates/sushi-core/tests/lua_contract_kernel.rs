use sushi_core::lua::contract::{ContractSchemaVersion, LuaCapabilityContract};

#[test]
fn contract_kernel_exports_v2_types() {
    let version = ContractSchemaVersion::V2;
    let contract = LuaCapabilityContract::default();
    assert_eq!(version.as_str(), "v2");
    assert!(contract.entries.is_empty());
}
