use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../")
        .canonicalize()
        .expect("failed to resolve repository root")
}

fn read(path: &str) -> String {
    std::fs::read_to_string(repo_root().join(path))
        .unwrap_or_else(|e| panic!("failed to read {path}: {e}"))
}

#[test]
fn base_shell_exposes_enterprise_landmarks() {
    let source = read("web/templates/base.html");
    assert!(source.contains("<body"));
    assert!(source.contains("<body data-admin-shell"));
    assert!(source.contains("class=\"admin-shell\" data-admin-shell"));
    assert!(source.contains("data-admin-nav"));
    assert!(source.contains("data-admin-workspace-stage"));
    assert!(source.contains("id=\"theme-toggle\""));
}

#[test]
fn base_shell_has_nav_and_stage_regions() {
    let source = read("web/templates/base.html");
    assert!(source.contains("data-admin-nav-section=\"primary\""));
    assert!(source.contains("data-admin-nav-section=\"system\""));
}

#[test]
fn core_admin_fragments_expose_page_header_contract() {
    let files = [
        "web/templates/admin/fragments/dashboard_content.html",
        "web/templates/admin/fragments/users_content.html",
        "web/templates/admin/fragments/roles_content.html",
        "web/templates/admin/fragments/permissions_content.html",
        "web/templates/admin/fragments/menus_content.html",
        "web/templates/admin/fragments/plugins_content.html",
        "web/templates/admin/fragments/logs_content.html",
        "web/templates/admin/fragments/config_content.html",
    ];
    for file in files {
        let source = read(file);
        assert!(source.contains("data-admin-page-header"), "missing in {file}");
        assert!(source.contains("data-admin-action-cluster"), "missing in {file}");
    }
}

#[test]
fn official_plugin_templates_follow_enterprise_workspace_contract() {
    let cms = read("plugins/official/cms/web/templates/cms.html");
    assert!(cms.contains("data-enterprise-workbench=\"cms\""));
    let kv = read("plugins/official/kv-store/web/templates/kv.html");
    assert!(kv.contains("data-enterprise-workbench=\"kv\""));
    let fb = read("plugins/official/file-browser/web/templates/file_browser.html");
    assert!(fb.contains("data-enterprise-workbench=\"file-browser\""));
}
