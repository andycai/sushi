use std::path::PathBuf;
use sushi_core::runtime::{
    AdminPageSpec, CapabilityRegistry, CliCommandSpec, HttpRouteSpec, HttpSurface,
    MenuContributionSpec, PluginInstanceId, RegistrationConflict, RegistrationSource,
    StaticRootSpec, TemplateRootSpec, TransportSpec,
};

fn owner(value: &str) -> PluginInstanceId {
    PluginInstanceId::new(value).expect("valid plugin instance id")
}

#[tokio::test]
async fn staged_registrations_are_invisible_until_commit() {
    let registry = CapabilityRegistry::new();
    let mut staged = registry.stage(owner("notes.default"));
    staged.register_http(HttpRouteSpec::new(
        "GET",
        "/api/notes",
        "notes",
        "handler::list",
    ));

    assert!(registry.snapshot().await.http_routes().is_empty());

    registry.commit(staged).await.expect("commit succeeds");

    let snapshot = registry.snapshot().await;
    assert_eq!(snapshot.http_routes().len(), 1);
    assert_eq!(snapshot.http_routes()[0].owner.as_str(), "notes.default");
}

#[tokio::test]
async fn transport_surfaces_follow_owner_lifecycle_and_conflict_rules() {
    let registry = CapabilityRegistry::new();
    let api_owner = owner("transport.api");
    let mut staged = registry.stage_with_source(api_owner.clone(), RegistrationSource::Builtin);
    staged.register_transport(TransportSpec::new(HttpSurface::Api));

    assert!(!registry.snapshot().await.has_transport(HttpSurface::Api));
    registry.commit(staged).await.expect("commit succeeds");
    assert!(registry.snapshot().await.has_transport(HttpSurface::Api));

    let mut conflicting =
        registry.stage_with_source(owner("transport.other"), RegistrationSource::Builtin);
    conflicting.register_transport(TransportSpec::new(HttpSurface::Api));
    let error = registry
        .commit(conflicting)
        .await
        .expect_err("only one owner may select a transport surface");
    assert!(matches!(error, RegistrationConflict::Transport { .. }));

    registry.remove_owner(&api_owner).await;
    assert!(!registry.snapshot().await.has_transport(HttpSurface::Api));
}

#[tokio::test]
async fn template_roots_follow_owner_commit_and_removal() {
    let registry = CapabilityRegistry::new();
    let plugin_owner = owner("notes.default");
    let mut staged = registry.stage(plugin_owner.clone());
    staged.register_template_root(
        TemplateRootSpec::new(
            "official/notes",
            PathBuf::from("/plugins/official/notes/web/templates"),
        )
        .unwrap(),
    );

    assert!(registry.snapshot().await.template_roots().is_empty());

    registry.commit(staged).await.expect("commit succeeds");
    let snapshot = registry.snapshot().await;
    assert_eq!(snapshot.template_roots().len(), 1);
    assert_eq!(
        snapshot.template_roots()[0].value.plugin_id.as_str(),
        "official/notes"
    );

    registry.remove_owner(&plugin_owner).await;
    assert!(registry.snapshot().await.template_roots().is_empty());
}

#[tokio::test]
async fn static_roots_follow_owner_commit_and_removal() {
    let registry = CapabilityRegistry::new();
    let plugin_owner = owner("notes.default");
    let mut staged = registry.stage(plugin_owner.clone());
    staged.register_static_root(
        StaticRootSpec::new(
            "official/notes",
            PathBuf::from("/plugins/official/notes/web/static"),
        )
        .unwrap(),
    );

    assert!(registry.snapshot().await.static_roots().is_empty());

    registry.commit(staged).await.expect("commit succeeds");
    let snapshot = registry.snapshot().await;
    assert_eq!(snapshot.static_roots().len(), 1);
    assert_eq!(
        snapshot.static_roots()[0].value.plugin_id.as_str(),
        "official/notes"
    );

    registry.remove_owner(&plugin_owner).await;
    assert!(registry.snapshot().await.static_roots().is_empty());
}

#[tokio::test]
async fn menu_contributions_follow_owner_commit_and_removal() {
    let registry = CapabilityRegistry::new();
    let plugin_owner = owner("notes.default");
    let mut staged = registry.stage(plugin_owner.clone());
    staged.register_menu(
        MenuContributionSpec::new("notes.main", "Notes", 40)
            .with_icon(Some("notebook".to_string()))
            .with_route(Some("/admin/notes".to_string()))
            .with_policy(Some("admin.notes.view".to_string())),
    );

    assert!(registry.snapshot().await.menu_contributions().is_empty());

    registry.commit(staged).await.expect("commit succeeds");
    let snapshot = registry.snapshot().await;
    assert_eq!(snapshot.menu_contributions().len(), 1);
    assert_eq!(snapshot.menu_contributions()[0].value.id, "notes.main");

    registry.remove_owner(&plugin_owner).await;
    assert!(registry.snapshot().await.menu_contributions().is_empty());
}

#[tokio::test]
async fn conflicting_menu_contribution_keeps_previous_snapshot_unchanged() {
    let registry = CapabilityRegistry::new();
    let mut first = registry.stage(owner("notes.default"));
    first.register_menu(MenuContributionSpec::new("workspace.notes", "Notes", 40));
    registry.commit(first).await.expect("first commit succeeds");

    let before = registry.snapshot().await;
    let mut conflicting = registry.stage(owner("cms.default"));
    conflicting.register_menu(MenuContributionSpec::new(
        "workspace.notes",
        "CMS Notes",
        50,
    ));

    let error = registry
        .commit(conflicting)
        .await
        .expect_err("duplicate menu contribution must fail closed");
    assert_eq!(
        error,
        RegistrationConflict::MenuContribution {
            id: "workspace.notes".to_string(),
            existing_owner: owner("notes.default"),
            incoming_owner: owner("cms.default"),
        }
    );
    assert_eq!(registry.snapshot().await, before);
}

#[tokio::test]
async fn conflicting_commit_keeps_previous_snapshot_unchanged() {
    let registry = CapabilityRegistry::new();

    let mut first = registry.stage(owner("notes.default"));
    first.register_http(HttpRouteSpec::new(
        "GET",
        "/api/notes",
        "notes",
        "handler::list",
    ));
    registry.commit(first).await.expect("first commit succeeds");

    let before = registry.snapshot().await;
    let mut conflicting = registry.stage(owner("cms.default"));
    conflicting.register_http(HttpRouteSpec::new(
        "GET",
        "/api/notes",
        "cms",
        "handler::notes",
    ));

    let err = registry
        .commit(conflicting)
        .await
        .expect_err("duplicate route must fail closed");
    assert_eq!(
        err,
        RegistrationConflict::HttpRoute {
            method: "GET".to_string(),
            path: "/api/notes".to_string(),
            existing_owner: owner("notes.default"),
            incoming_owner: owner("cms.default"),
        }
    );

    let after = registry.snapshot().await;
    assert_eq!(before, after);
}

#[tokio::test]
async fn same_owner_can_replace_an_existing_registration() {
    let registry = CapabilityRegistry::new();
    let plugin_owner = owner("notes.default");

    let mut first = registry.stage(plugin_owner.clone());
    first.register_http(HttpRouteSpec::new(
        "GET",
        "/api/notes",
        "notes",
        "handler::old",
    ));
    registry.commit(first).await.expect("first commit succeeds");

    let mut replacement = registry.stage(plugin_owner);
    replacement.register_http(
        HttpRouteSpec::new("GET", "/api/notes", "notes", "handler::new")
            .with_policy(Some("api.notes.read".to_string())),
    );
    registry
        .commit(replacement)
        .await
        .expect("same owner replacement succeeds");

    let snapshot = registry.snapshot().await;
    assert_eq!(snapshot.http_routes().len(), 1);
    assert_eq!(snapshot.http_routes()[0].value.handler_key, "handler::new");
    assert_eq!(
        snapshot.http_routes()[0].value.policy_key.as_deref(),
        Some("api.notes.read")
    );
}

#[tokio::test]
async fn removing_owner_atomically_removes_all_capabilities() {
    let registry = CapabilityRegistry::new();
    let plugin_owner = owner("notes.default");
    let mut staged = registry.stage(plugin_owner.clone());
    staged.register_http(HttpRouteSpec::new(
        "GET",
        "/api/notes",
        "notes",
        "handler::list",
    ));
    staged.register_admin(AdminPageSpec::new(
        "/admin/notes",
        "Notes",
        "notes",
        "handler::admin",
    ));
    staged.register_cli(CliCommandSpec::new(
        "notes-list",
        "List notes",
        "notes",
        "handler::cli",
    ));
    registry.commit(staged).await.expect("commit succeeds");

    registry.remove_owner(&plugin_owner).await;

    let snapshot = registry.snapshot().await;
    assert!(snapshot.http_routes().is_empty());
    assert!(snapshot.admin_pages().is_empty());
    assert!(snapshot.cli_commands().is_empty());
}

#[tokio::test]
async fn inspection_order_is_stable_across_registration_order() {
    let registry = CapabilityRegistry::new();
    let mut staged = registry.stage(owner("notes.default"));
    staged.register_cli(CliCommandSpec::new("z-last", "Last", "notes", "handler::z"));
    staged.register_http(HttpRouteSpec::new(
        "POST",
        "/api/notes",
        "notes",
        "handler::post",
    ));
    staged.register_admin(AdminPageSpec::new(
        "/admin/notes",
        "Notes",
        "notes",
        "handler::admin",
    ));
    staged.register_http(HttpRouteSpec::new(
        "GET",
        "/api/notes",
        "notes",
        "handler::get",
    ));
    staged.register_cli(CliCommandSpec::new(
        "a-first",
        "First",
        "notes",
        "handler::a",
    ));
    staged.register_menu(MenuContributionSpec::new("notes.main", "Notes", 40));
    registry.commit(staged).await.expect("commit succeeds");

    let entries = registry.snapshot().await.inspect();
    let keys = entries
        .into_iter()
        .map(|entry| entry.key)
        .collect::<Vec<_>>();

    assert_eq!(
        keys,
        vec![
            "admin:/admin/notes",
            "cli:a-first",
            "cli:z-last",
            "http:api:GET:/api/notes",
            "http:api:POST:/api/notes",
            "menu:notes.main",
        ]
    );
}

#[tokio::test]
async fn http_matcher_isolated_by_explicit_surface() {
    let registry = CapabilityRegistry::new();
    let mut staged = registry.stage(owner("notes.default"));
    staged.register_http(HttpRouteSpec::new(
        "GET",
        "/admin/partials/notes",
        "notes",
        "handler::partial",
    ));
    registry.commit(staged).await.expect("commit succeeds");

    let snapshot = registry.snapshot().await;
    assert!(snapshot
        .match_http_on(HttpSurface::Api, "GET", "/admin/partials/notes")
        .is_none());
    assert!(snapshot
        .match_http_on(HttpSurface::Admin, "GET", "/admin/partials/notes")
        .is_some());
}

#[tokio::test]
async fn parameterized_http_routes_match_literal_request_paths() {
    let registry = CapabilityRegistry::new();
    let mut staged = registry.stage_with_source(owner("host.admin"), RegistrationSource::Builtin);
    staged.register_http(HttpRouteSpec::new(
        "GET",
        "/admin/plugins/{plugin}",
        "host-admin",
        "rust::plugin-workspace",
    ));
    registry.commit(staged).await.expect("commit succeeds");

    let snapshot = registry.snapshot().await;
    let registration = snapshot
        .match_http_on(HttpSurface::Admin, "GET", "/admin/plugins/cms")
        .expect("parameterized route should match");
    assert_eq!(registration.value.handler_key, "rust::plugin-workspace");
}

#[tokio::test]
async fn parameterized_admin_pages_match_literal_request_paths() {
    let registry = CapabilityRegistry::new();
    let mut staged = registry.stage_with_source(owner("host.admin"), RegistrationSource::Builtin);
    staged.register_admin(AdminPageSpec::new(
        "/admin/plugins/{plugin}",
        "Plugin Workspace",
        "host-admin",
        "rust::plugin-workspace",
    ));
    registry.commit(staged).await.expect("commit succeeds");

    let snapshot = registry.snapshot().await;
    let registration = snapshot
        .admin_page("/admin/plugins/cms")
        .expect("parameterized admin page should match");
    assert_eq!(registration.value.handler_key, "rust::plugin-workspace");
}

#[tokio::test]
async fn http_match_precedence_is_exact_then_parameter_then_catch_all() {
    let registry = CapabilityRegistry::new();
    let mut staged = registry.stage_with_source(owner("host.admin"), RegistrationSource::Builtin);
    staged.register_http(HttpRouteSpec::new(
        "GET",
        "/admin/plugins/*",
        "host-admin",
        "rust::catch-all",
    ));
    staged.register_http(HttpRouteSpec::new(
        "GET",
        "/admin/plugins/{plugin}",
        "host-admin",
        "rust::parameter",
    ));
    staged.register_http(HttpRouteSpec::new(
        "GET",
        "/admin/plugins/settings",
        "host-admin",
        "rust::exact",
    ));
    registry.commit(staged).await.expect("commit succeeds");

    let snapshot = registry.snapshot().await;
    assert_eq!(
        snapshot
            .match_http_on(HttpSurface::Admin, "GET", "/admin/plugins/settings")
            .unwrap()
            .value
            .handler_key,
        "rust::exact"
    );
    assert_eq!(
        snapshot
            .match_http_on(HttpSurface::Admin, "GET", "/admin/plugins/cms")
            .unwrap()
            .value
            .handler_key,
        "rust::parameter"
    );
    assert_eq!(
        snapshot
            .match_http_on(HttpSurface::Admin, "GET", "/admin/plugins/cms/pages")
            .unwrap()
            .value
            .handler_key,
        "rust::catch-all"
    );
}

#[tokio::test]
async fn http_surface_must_match_route_path() {
    let registry = CapabilityRegistry::new();
    let plugin_owner = owner("notes.default");
    let mut staged = registry.stage(plugin_owner.clone());
    staged.register_http(
        HttpRouteSpec::new("GET", "/api/notes", "notes", "handler::list")
            .with_surface(HttpSurface::Admin),
    );

    let error = registry
        .commit(staged)
        .await
        .expect_err("surface mismatch must fail closed");
    assert_eq!(
        error,
        RegistrationConflict::HttpSurfacePath {
            surface: "admin".to_string(),
            method: "GET".to_string(),
            path: "/api/notes".to_string(),
            owner: plugin_owner,
        }
    );
}

#[tokio::test]
async fn lua_route_cannot_shadow_reserved_host_route() {
    let registry = CapabilityRegistry::new();
    let plugin_owner = owner("notes.default");
    let mut staged = registry.stage_with_source(plugin_owner.clone(), RegistrationSource::Lua);
    staged.register_http(HttpRouteSpec::new(
        "GET",
        "/api/users",
        "notes",
        "handler::users",
    ));

    let error = registry
        .commit(staged)
        .await
        .expect_err("reserved Host route must fail closed");
    assert_eq!(
        error,
        RegistrationConflict::ReservedHttpRoute {
            registration_source: RegistrationSource::Lua,
            method: "GET".to_string(),
            path: "/api/users".to_string(),
            owner: plugin_owner,
            reserved_method: "GET".to_string(),
            reserved_path: "/api/users".to_string(),
        }
    );
}

#[tokio::test]
async fn wildcard_shadow_diagnostic_uses_stable_reserved_path() {
    let registry = CapabilityRegistry::new();
    let plugin_owner = owner("catch-all.default");
    let mut staged = registry.stage_with_source(plugin_owner.clone(), RegistrationSource::Lua);
    staged.register_http(HttpRouteSpec::new(
        "GET",
        "/api/*",
        "catch-all",
        "handler::all",
    ));

    let error = registry
        .commit(staged)
        .await
        .expect_err("wildcard overlap must fail closed");
    assert_eq!(
        error,
        RegistrationConflict::ReservedHttpRoute {
            registration_source: RegistrationSource::Lua,
            method: "GET".to_string(),
            path: "/api/*".to_string(),
            owner: plugin_owner,
            reserved_method: "GET".to_string(),
            reserved_path: "/api/auth/me".to_string(),
        }
    );
}

#[tokio::test]
async fn builtin_route_can_take_over_reserved_host_route() {
    let registry = CapabilityRegistry::new();
    let mut staged = registry.stage_with_source(owner("host.api"), RegistrationSource::Builtin);
    staged.register_http(HttpRouteSpec::new(
        "GET",
        "/api/users",
        "host-api",
        "rust::users-list",
    ));

    registry
        .commit(staged)
        .await
        .expect("builtin route may replace static Host implementation");
    let snapshot = registry.snapshot().await;
    let registration = snapshot.match_http("GET", "/api/users").unwrap();
    assert_eq!(registration.source, RegistrationSource::Builtin);
}

#[tokio::test]
async fn non_builtin_cli_command_cannot_claim_host_reserved_name() {
    let registry = CapabilityRegistry::new();
    let plugin_owner = owner("notes.default");
    let mut staged = registry.stage_with_source(plugin_owner.clone(), RegistrationSource::Lua);
    staged.register_cli(CliCommandSpec::new(
        "serve",
        "Shadow the host launcher",
        "notes",
        "handler::serve",
    ));

    let error = registry
        .commit(staged)
        .await
        .expect_err("Lua plugins must not claim Host CLI names");
    assert_eq!(
        error,
        RegistrationConflict::ReservedCliCommand {
            registration_source: RegistrationSource::Lua,
            name: "serve".to_string(),
            owner: plugin_owner,
        }
    );
}

#[tokio::test]
async fn builtin_cli_command_can_claim_host_reserved_name() {
    let registry = CapabilityRegistry::new();
    let mut staged = registry.stage_with_source(owner("host.cli"), RegistrationSource::Builtin);
    staged.register_cli(CliCommandSpec::new(
        "doctor",
        "Diagnose the runtime",
        "host-cli",
        "builtin::doctor",
    ));

    registry
        .commit(staged)
        .await
        .expect("Host Builtin may register its reserved CLI name");
    assert_eq!(
        registry
            .snapshot()
            .await
            .cli_command("doctor")
            .unwrap()
            .source,
        RegistrationSource::Builtin
    );
}

#[tokio::test]
async fn plugin_route_adjacent_to_reserved_host_route_is_allowed() {
    let registry = CapabilityRegistry::new();
    let mut staged = registry.stage_with_source(owner("notes.default"), RegistrationSource::Lua);
    staged.register_http(HttpRouteSpec::new(
        "GET",
        "/api/users-notes",
        "notes",
        "handler::list",
    ));

    registry
        .commit(staged)
        .await
        .expect("adjacent non-overlapping route should remain available");
}

#[tokio::test]
async fn lua_admin_page_cannot_shadow_reserved_host_page() {
    let registry = CapabilityRegistry::new();
    let plugin_owner = owner("notes.default");
    let mut staged = registry.stage_with_source(plugin_owner.clone(), RegistrationSource::Lua);
    staged.register_admin(AdminPageSpec::new(
        "/admin/users",
        "Notes",
        "notes",
        "handler::admin",
    ));

    let error = registry
        .commit(staged)
        .await
        .expect_err("reserved Host admin page must fail closed");
    assert_eq!(
        error,
        RegistrationConflict::ReservedHttpRoute {
            registration_source: RegistrationSource::Lua,
            method: "GET".to_string(),
            path: "/admin/users".to_string(),
            owner: plugin_owner,
            reserved_method: "GET".to_string(),
            reserved_path: "/admin/users".to_string(),
        }
    );
}
