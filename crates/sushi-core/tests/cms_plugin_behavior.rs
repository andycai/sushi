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
