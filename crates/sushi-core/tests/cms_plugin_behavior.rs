use std::path::Path;

fn repo_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

#[test]
fn cms_soft_deleted_posts_are_hidden_from_list() {
    let source = std::fs::read_to_string(
        repo_root().join("plugins/official/cms/lua/domain/post.lua"),
    )
    .expect("failed to read post domain");

    assert!(source.contains("function post.list"));
    assert!(source.contains("p.deleted_at IS NULL"));
    assert!(source.contains("p.status = 'published'"));
}

#[test]
fn cms_category_delete_conflicts_when_posts_exist() {
    let source = std::fs::read_to_string(
        repo_root().join("plugins/official/cms/lua/domain/category.lua"),
    )
    .expect("failed to read category domain");

    assert!(source.contains("function category.soft_delete"));
    assert!(source.contains("SELECT id FROM cms_posts"));
    assert!(source.contains("conflict_has_posts"));
}

#[test]
fn cms_public_page_route_hides_draft_content() {
    let source = std::fs::read_to_string(
        repo_root().join("plugins/official/cms/lua/interfaces/api.lua"),
    )
    .expect("failed to read api interface");

    assert!(source.contains("function api.public_page_detail"));
    assert!(source.contains("page.get_by_slug(slug, { only_published = true })"));
    assert!(source.contains("return json_error(kind, msg)"));
}

#[test]
fn cms_post_list_category_query_filters_rows() {
    let source = std::fs::read_to_string(
        repo_root().join("plugins/official/cms/lua/interfaces/api.lua"),
    )
    .expect("failed to read api interface");

    assert!(source.contains("function api.public_post_list"));
    assert!(source.contains("path:match(\"[?&]category=([^&]+)\")"));
    assert!(source.contains("post.list({ only_published = true, category_slug = category_slug })"));
}

#[test]
fn cms_cli_dispatch_supports_page_list() {
    let source = std::fs::read_to_string(
        repo_root().join("plugins/official/cms/lua/interfaces/cli.lua"),
    )
    .expect("failed to read cli interface");

    assert!(source.contains("function cli.cms_dispatch"));
    assert!(source.contains("resource == \"page\" and action == \"list\""));
    assert!(source.contains("rows[i].slug .. \" [\" .. rows[i].status .. \"]\""));
}

#[test]
fn cms_public_post_detail_hides_draft_posts() {
    let source = std::fs::read_to_string(
        repo_root().join("plugins/official/cms/lua/interfaces/api.lua"),
    )
    .expect("failed to read api interface");

    assert!(source.contains("function api.public_post_detail"));
    assert!(source.contains("post.get_by_slug(slug, { only_published = true })"));
    assert!(source.contains("return json_error(kind, msg)"));
}

#[test]
fn cms_page_domain_exposes_overview_and_status_helpers() {
    let source = std::fs::read_to_string(
        repo_root().join("plugins/official/cms/lua/domain/page.lua"),
    )
    .expect("failed to read page domain");

    assert!(source.contains("function page.count_by_status"));
    assert!(source.contains("function page.recent"));
    assert!(source.contains("function page.set_status"));
    assert!(source.contains("validate.validate_status(status)"));
    assert!(source.contains("slug.normalize(slug_input)"));
    assert!(source.contains("slug cannot be empty"));
    assert!(source.contains("limit:match(\"^%d+$\")"));
    assert!(source.contains("max ~= math.floor(max)"));
    assert!(source.contains("SAFE_INTEGER_MAX"));
    assert!(source.contains("max > SAFE_INTEGER_MAX"));
}

#[test]
fn cms_post_domain_exposes_overview_and_status_helpers() {
    let source = std::fs::read_to_string(
        repo_root().join("plugins/official/cms/lua/domain/post.lua"),
    )
    .expect("failed to read post domain");

    assert!(source.contains("function post.count_by_status"));
    assert!(source.contains("function post.recent"));
    assert!(source.contains("function post.set_status"));
    assert!(source.contains("validate.validate_status(status)"));
    assert!(source.contains("slug.normalize(slug_input)"));
    assert!(source.contains("slug cannot be empty"));
    assert!(source.contains("limit:match(\"^%d+$\")"));
    assert!(source.contains("max ~= math.floor(max)"));
    assert!(source.contains("SAFE_INTEGER_MAX"));
    assert!(source.contains("max > SAFE_INTEGER_MAX"));
}

#[test]
fn cms_admin_interface_exposes_workbench_handlers() {
    let source = std::fs::read_to_string(
        repo_root().join("plugins/official/cms/lua/interfaces/admin.lua"),
    )
    .expect("failed to read admin interface");

    assert!(source.contains("function admin.overview_partial"));
    assert!(source.contains("function admin.library_partial"));
    assert!(source.contains("function admin.editor_partial"));
    assert!(source.contains("function admin.editor_save_partial"));
    assert!(source.contains("function admin.status_transition_partial"));
    assert!(source.contains("function admin.commands_partial"));

    assert!(source.contains("plugins/official/cms/fragments/overview_panel.html"));
    assert!(source.contains("page.count_by_status()"));
    assert!(source.contains("post.count_by_status()"));
    assert!(source.contains("page.recent(5)"));
    assert!(source.contains("post.recent(5)"));
    assert!(source.contains("path:match(\"^/admin/partials/cms/library/([^/?]+)\")"));
    assert!(source.contains("path:match(\"^/admin/partials/cms/editor/([^/?]+)\")"));
    assert!(source.contains("kind == \"page\" or resource == \"page\""));
    assert!(source.contains("kind == \"post\" or resource == \"post\""));
    assert!(source.contains("resource == \"categories\""));
    assert!(source.contains("category.upsert({"));
    assert!(source.contains("form.content_type"));
    assert!(source.contains("page.set_status("));
    assert!(source.contains("post.set_status("));
    assert!(source.contains("cms_overview_template_fallback_marker"));
    assert!(source.contains("pcall(sushi.web.render, \"plugins/official/cms/fragments/overview_panel.html\", data)"));
}
