use std::path::Path;

use sushi_core::auth::jwt::JwtService;
use sushi_core::config::{ConfigStore, SushiConfig};
use sushi_core::context::SushiContext;
use sushi_core::runtime::{CapabilityRegistry, PluginInstanceId, TemplateRootSpec};
use sushi_core::storage::sqlite::SqliteStorage;
use sushi_core::web::template_error::TemplateError;
use sushi_core::web::template_service::TemplateService;
use tokio::runtime::Runtime;

#[test]
fn render_with_inheritance_works() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("admin")).unwrap();
    std::fs::write(
        root.path().join("base.html"),
        "<html>{% block body %}{% endblock %}</html>",
    )
    .unwrap();
    std::fs::write(
        root.path().join("admin/login.html"),
        "{% extends \"base.html\" %}{% block body %}{{ title }}{% endblock %}",
    )
    .unwrap();

    let svc = TemplateService::new(root.path()).unwrap();
    let html = svc
        .render("admin/login.html", serde_json::json!({"title": "Login"}))
        .unwrap();
    assert_eq!(html, "<html>Login</html>");
}

#[test]
fn new_fails_when_root_missing() {
    let temp_dir = tempfile::tempdir().unwrap();
    let missing = temp_dir.path().join("not_found");
    let result = TemplateService::new(&missing);
    match result {
        Err(TemplateError::RootMissing { path }) => {
            assert!(path.contains("not_found"));
        }
        Err(other) => panic!("unexpected error variant: {other:?}"),
        Ok(_) => panic!("expected TemplateService::new to fail when root missing"),
    }
}

#[test]
fn render_missing_template_errors() {
    let root = tempfile::tempdir().unwrap();
    let service = TemplateService::new(root.path()).unwrap();
    let err = service
        .render("does-not-exist.html", serde_json::json!({}))
        .unwrap_err();
    assert!(matches!(
        err,
        TemplateError::TemplateLoad { path, .. } if path == "does-not-exist.html"
    ));
}

#[test]
fn context_can_hold_template_service() {
    let runtime = Runtime::new().unwrap();
    let storage = runtime.block_on(SqliteStorage::new_in_memory()).unwrap();
    let config = ConfigStore::new(SushiConfig::default());
    let jwt = JwtService::new("test-secret-key-at-least-32-chars-long!", 3600, 604800);

    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("base.html"), "ok").unwrap();
    let templates = TemplateService::new(root.path()).unwrap();

    let _ = SushiContext::new(config, storage, jwt, templates);
}

#[test]
fn base_template_uses_local_assets_only() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root missing");
    let templates_dir = workspace_root.join("web").join("templates");

    let svc = TemplateService::new(&templates_dir).unwrap();
    let html = svc.render("base.html", serde_json::json!({})).unwrap();

    assert!(html.contains("js/alpine.min.js"));
    assert!(html.contains("js/htmx.min.js"));
    assert!(html.contains("css/style.css"));
    assert!(
        !html.contains("https://"),
        "base template should not reference https:// resources"
    );
    assert!(
        !html.contains("http://"),
        "base template should not reference http:// resources"
    );
    assert!(
        !html.contains("src=\"//"),
        "base template should not reference protocol-relative // resources"
    );
}

#[test]
fn base_template_uses_svg_favicon_only() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root missing");
    let templates_dir = workspace_root.join("web").join("templates");

    let svc = TemplateService::new(&templates_dir).unwrap();
    let html = svc.render("base.html", serde_json::json!({})).unwrap();

    assert!(
        html.contains("rel=\"icon\""),
        "base template should include favicon rel"
    );
    assert!(
        html.contains("image/svg+xml"),
        "base template should set svg favicon mime type"
    );
    assert!(
        html.contains("favicon.svg"),
        "base template should include svg favicon path"
    );
    assert!(
        !html.contains("favicon.png"),
        "base template should not reference favicon.png"
    );
}

#[test]
fn render_plugin_template_from_tiered_plugin_template_root() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(
        root.path().join("base.html"),
        "<html>{% block body %}{% endblock %}</html>",
    )
    .unwrap();

    let plugin_templates = tempfile::tempdir().unwrap();
    std::fs::write(
        plugin_templates.path().join("page.html"),
        "{% extends \"base.html\" %}{% block body %}Plugin {{ title }}{% endblock %}",
    )
    .unwrap();

    let svc = TemplateService::new_with_plugin_roots(
        root.path(),
        vec![(
            "official/kv-store".to_string(),
            plugin_templates.path().to_path_buf(),
        )],
    )
    .unwrap();

    let html = svc
        .render(
            "plugins/official/kv-store/page.html",
            serde_json::json!({"title": "Workspace"}),
        )
        .unwrap();

    assert_eq!(html, "<html>Plugin Workspace</html>");
}

#[test]
fn dynamic_plugin_template_root_disappears_after_owner_removal() {
    let runtime = Runtime::new().unwrap();
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("base.html"), "<html>{{ value }}</html>").unwrap();
    let plugin_templates = tempfile::tempdir().unwrap();
    std::fs::write(plugin_templates.path().join("page.html"), "Plugin {{ value }}").unwrap();

    let registry = CapabilityRegistry::new();
    let owner = PluginInstanceId::new("notes.default").unwrap();
    let mut staged = registry.stage(owner.clone());
    staged.register_template_root(TemplateRootSpec::new(
        "official/notes",
        plugin_templates.path().to_path_buf(),
    ));
    runtime.block_on(registry.commit(staged)).unwrap();

    let service = TemplateService::new_with_registry(root.path(), registry.clone()).unwrap();
    let name = "plugins/official/notes/page.html";
    assert_eq!(
        service
            .render(name, serde_json::json!({"value": "visible"}))
            .unwrap(),
        "Plugin visible"
    );

    runtime.block_on(registry.remove_owner(&owner));
    assert!(service
        .render(name, serde_json::json!({"value": "hidden"}))
        .is_err());
}
