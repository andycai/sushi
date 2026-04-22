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
    assert!(source.contains("class=\"admin-shell\""));
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
fn users_fragment_has_enterprise_toolbar_and_table_card() {
    let files = [
        "web/templates/admin/fragments/users_content.html",
        "web/templates/admin/fragments/roles_content.html",
        "web/templates/admin/fragments/permissions_content.html",
        "web/templates/admin/fragments/menus_content.html",
        "web/templates/admin/fragments/plugins_content.html",
        "web/templates/admin/fragments/logs_content.html",
    ];
    for file in files {
        let source = read(file);
        assert!(source.contains("data-admin-table-card"), "missing in {file}");
    }
}

#[test]
fn file_browser_template_exposes_workspace_bootstrap_contract() {
    let fb = read("plugins/official/file-browser/web/templates/file_browser.html");
    assert!(fb.contains("id=\"theme-toggle\""));
    assert!(fb.contains("fileBrowserPage("));
}

#[test]
fn file_browser_workbench_uses_enterprise_workspace_semantics() {
    let source = read("plugins/official/file-browser/web/templates/file_browser.html");
    assert!(source.contains("data-enterprise-workbench=\"file-browser\""));
    assert!(source.contains("data-admin-workspace-module=\"file-browser\""));
    assert!(source.contains("data-admin-page-header"));
    assert!(source.contains("data-admin-action-cluster"));
}

#[test]
fn file_browser_fragments_preserve_hooks_and_enterprise_tone() {
    let list = read("plugins/official/file-browser/web/templates/fragments/list.html");
    assert!(list.contains("data-admin-table-card"));
    assert!(list.contains("data-fb-list-root="));
    assert!(list.contains("data-fb-node=\"1\""));
    assert!(list.contains("data-fb-children-for="));

    let editor = read("plugins/official/file-browser/web/templates/fragments/editor.html");
    assert!(editor.contains("data-fb-action=\"refresh-list\""));
    assert!(editor.contains("data-fb-action=\"save-form\""));
    assert!(editor.contains("data-fb-action=\"download\""));

    let flash = read("plugins/official/file-browser/web/templates/fragments/flash.html");
    assert!(flash.contains("class=\"alert"));
    assert!(flash.contains("role=\"alert\""));
    assert!(flash.contains("{{ message }}"));
}

#[test]
fn cms_workbench_uses_enterprise_workspace_semantics() {
    let source = read("plugins/official/cms/web/templates/cms.html");
    assert!(source.contains("data-enterprise-workbench=\"cms\""));
    assert!(source.contains("data-admin-page-header"));
    assert!(source.contains("data-admin-action-cluster"));
}

#[test]
fn kv_workbench_uses_enterprise_workspace_semantics() {
    let source = read("plugins/official/kv-store/web/templates/fragments/kv_content.html");
    assert!(source.contains("data-enterprise-workbench=\"kv\""));
    assert!(source.contains("data-admin-page-header"));
    assert!(source.contains("class=\"mb-5"));
    assert!(source.contains("data-admin-action-cluster"));
    assert!(source.contains("data-admin-table-card"));
    assert!(source.contains("x-data=\"kvPage()\""));
    assert!(source.contains("hx-get=\"/admin/partials/kv/table\""));
    assert!(source.contains("hx-trigger=\"load, kv:refresh from:body\""));
}

#[test]
fn kv_partials_preserve_row_and_flash_contracts() {
    let rows = read("plugins/official/kv-store/web/templates/partials/rows.html");
    assert!(rows.contains("data-row-search="));
    assert!(rows.contains("data-row-sort="));
    assert!(rows.contains("@click=\"openEdit("));
    assert!(rows.contains("@click=\"openDeleteConfirm("));

    let flash = read("plugins/official/kv-store/web/templates/partials/flash.html");
    assert!(flash.contains("data-ui-flash"));
    assert!(flash.contains("data-level"));
    assert!(flash.contains("data-message"));
    assert!(flash.contains("role=\"alert\""));
}

#[test]
fn cms_fragments_expose_enterprise_header_and_row_contracts() {
    let library = read("plugins/official/cms/web/templates/fragments/library_panel.html");
    assert!(library.contains("data-admin-page-header"));
    assert!(library.contains("data-admin-action-cluster"));
    assert!(library.contains("data-admin-table-card"));

    let editor = read("plugins/official/cms/web/templates/fragments/editor_panel.html");
    assert!(editor.contains("data-admin-page-header"));
    assert!(editor.contains("data-admin-action-cluster"));

    let post_rows = read("plugins/official/cms/web/templates/fragments/post_rows.html");
    assert!(post_rows.contains("data-cms-row"));
    assert!(post_rows.contains("data-resource=\"posts\""));

    let page_rows = read("plugins/official/cms/web/templates/fragments/page_rows.html");
    assert!(page_rows.contains("data-cms-row"));
    assert!(page_rows.contains("data-resource=\"pages\""));

    let category_rows = read("plugins/official/cms/web/templates/fragments/category_rows.html");
    assert!(category_rows.contains("data-cms-row"));
    assert!(category_rows.contains("data-resource=\"categories\""));
}
