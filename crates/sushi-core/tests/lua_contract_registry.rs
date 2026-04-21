use sushi_core::lua::contract::schema::api::ApiRouteContract;
use sushi_core::lua::errors::LuaContractErrorCode;
use sushi_core::lua::registry::CapabilityRegistry;

#[test]
fn registry_stores_api_metadata() {
    let mut registry = CapabilityRegistry::new();
    registry
        .register_api(ApiRouteContract {
            method: "GET".to_string(),
            path: "/health".to_string(),
            handler_key: "plugin.health_handler".to_string(),
            policy: Some("api.health.read".to_string()),
            public: false,
        })
        .expect("route metadata should register");

    let snapshot = registry.snapshot();
    assert_eq!(snapshot.api_routes.len(), 1);

    let route = &snapshot.api_routes[0];
    assert_eq!(route.method, "GET");
    assert_eq!(route.path, "/health");
    assert_eq!(route.handler_key, "plugin.health_handler");
    assert_eq!(route.policy.as_deref(), Some("api.health.read"));
    assert!(!route.public);
}

#[test]
fn registry_rejects_public_policy_conflict() {
    let mut registry = CapabilityRegistry::new();
    let err = registry
        .register_api(ApiRouteContract {
            method: "GET".to_string(),
            path: "/public".to_string(),
            handler_key: "plugin.public_handler".to_string(),
            policy: Some("api.public.read".to_string()),
            public: true,
        })
        .expect_err("public routes cannot define policy");

    assert_eq!(err.code(), LuaContractErrorCode::PublicPolicyConflict);
}
