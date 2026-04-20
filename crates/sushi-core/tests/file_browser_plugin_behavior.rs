use std::path::{Path, PathBuf};

use sushi_core::auth::jwt::JwtService;
use sushi_core::config::{ConfigStore, SushiConfig};
use sushi_core::context::SushiContext;
use sushi_core::lua::loader::LuaPlugin;
use sushi_core::plugin::Plugin;
use sushi_core::storage::sqlite::SqliteStorage;
use sushi_core::web::template_service::TemplateService;

fn copy_dir_all(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).expect("create destination directory");
    for entry in std::fs::read_dir(src).expect("read source directory") {
        let entry = entry.expect("read directory entry");
        let source = entry.path();
        let target = dst.join(entry.file_name());
        if source.is_dir() {
            copy_dir_all(&source, &target);
        } else {
            std::fs::copy(&source, &target).expect("copy file");
        }
    }
}

fn write_manifest(path: &Path, root_path: &Path) {
    let source = format!(
        r#"
[plugin]
name = "file-browser"
version = "0.1.0"
description = "Public web file browser"
entry = "init.lua"
kind = "official"

[permissions]
routes = true
commands = false
admin = false
database = false

[file_browser]
route_prefix = "/app/files"
hide_dotfiles = true
deny_symlink = true
text_extensions = ["txt", "md", "json", "toml", "yaml", "yml", "lua", "rs", "js", "ts", "html", "css"]

[[file_browser.roots]]
id = "docs"
title = "Documents"
path = "{}"

[file_browser.roots.capabilities]
can_list = true
can_view_text = true
can_edit_text = true
can_create_text = true
can_create_dir = true
can_rename = true
can_delete = true
can_upload = true
can_download = true
"#,
        root_path.display()
    );
    std::fs::write(path, source).expect("write plugin manifest");
}

async fn dispatch(
    ctx: &SushiContext,
    method: &str,
    path: &str,
    dispatch_path: &str,
    body: Option<Vec<u8>>,
) -> String {
    let result = ctx
        .plugins
        .dispatch_api_handler(method, path, dispatch_path, body)
        .await;
    match result {
        Some(Ok(body)) => body,
        Some(Err(err)) => panic!("plugin dispatch failed: {err}"),
        None => panic!("route {method} {path} not found"),
    }
}

#[tokio::test]
async fn file_browser_public_routes_support_core_operations() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let source_plugin = repo_root.join("plugins/official/file-browser");

    let sandbox = tempfile::tempdir().expect("create sandbox dir");
    let plugin_root = sandbox.path().join("official").join("file-browser");
    std::fs::create_dir_all(&plugin_root).expect("create plugin root");

    copy_dir_all(&source_plugin.join("lua"), &plugin_root.join("lua"));
    copy_dir_all(&source_plugin.join("web"), &plugin_root.join("web"));
    std::fs::copy(source_plugin.join("init.lua"), plugin_root.join("init.lua"))
        .expect("copy init.lua");

    let docs_root = sandbox.path().join("docs-root");
    std::fs::create_dir_all(&docs_root).expect("create docs root");
    std::fs::write(docs_root.join("welcome.txt"), "hello").expect("write text fixture");
    write_manifest(&plugin_root.join("plugin.toml"), &docs_root);

    let mut plugins = LuaPlugin::scan_dir(sandbox.path())
        .await
        .expect("scan plugins");
    assert_eq!(plugins.len(), 1);

    let plugin = plugins.remove(0);
    let templates_root = tempfile::tempdir().expect("create templates root");
    let templates = TemplateService::new_with_plugin_roots(
        templates_root.path(),
        vec![(plugin.path_id().to_string(), plugin.web_templates_dir())],
    )
    .expect("create template service");

    let config = ConfigStore::new(SushiConfig::default());
    let db = SqliteStorage::new_in_memory()
        .await
        .expect("create sqlite db");
    db.run_migrations(include_str!("../../../migrations/001_init.sql"))
        .await
        .expect("run migration 001");
    db.run_migrations(include_str!("../../../migrations/003_rbac.sql"))
        .await
        .expect("run migration 003");
    db.run_migrations(include_str!(
        "../../../migrations/006_unified_policy_v2.sql"
    ))
    .await
    .expect("run migration 006");
    db.run_migrations(include_str!(
        "../../../migrations/008_plugin_governance_v1.sql"
    ))
    .await
    .expect("run migration 008");
    let jwt = JwtService::new("test-secret-key-at-least-32-chars-long!", 3600, 604800);
    let ctx = SushiContext::new(config, db, jwt, templates);

    ctx.plugins
        .register_plugin_manifest_with_permissions(
            plugin.manifest(),
            plugin.effective_permissions(),
        )
        .await;

    plugin.init(&ctx).await.expect("init plugin");
    let lua_vm = plugin.into_vm().expect("extract plugin vm");
    ctx.plugins.register_vm("file-browser", lua_vm).await;

    let html = dispatch(&ctx, "GET", "/app/files", "/app/files", None).await;
    assert!(html.contains("Official File Browser"));
    assert!(html.contains("file_browser.css?v="));
    assert!(!html.contains("tailwindcss-4.2.2.js"));
    assert!(html.contains("fb-context-menu"));
    assert!(html.contains("Right click a folder in tree"));
    assert!(html.contains("ctx-upload"));
    assert!(html.contains("quick-create-text"));
    assert!(html.contains("data-fb-toolbar=\"1\""));
    assert!(!html.contains("x-text=\"relPath === '' ? '/' : relPath\""));
    assert!(!html.contains("x-init=\"init()\""));
    assert!(!html.contains("@submit.prevent=\"uploadFile($event)\""));

    let create_dir_body = b"root_id=docs&parent_path=&name=notes".to_vec();
    let flash = dispatch(
        &ctx,
        "POST",
        "/app/files/create-dir",
        "/app/files/create-dir",
        Some(create_dir_body),
    )
    .await;
    assert!(flash.contains("Created directory notes"));

    let create_text_body =
        b"root_id=docs&parent_path=notes&name=todo&initial_content=first".to_vec();
    let flash = dispatch(
        &ctx,
        "POST",
        "/app/files/create-text",
        "/app/files/create-text",
        Some(create_text_body),
    )
    .await;
    assert!(flash.contains("Created todo.txt"));

    let save_path = "/app/files/save/docs";
    let save_dispatch = "/app/files/save/docs?path=notes/todo.txt";
    let flash = dispatch(
        &ctx,
        "POST",
        save_path,
        save_dispatch,
        Some(b"updated content".to_vec()),
    )
    .await;
    assert!(flash.contains("Saved todo.txt"));

    let list_html = dispatch(
        &ctx,
        "GET",
        "/app/files/list/docs",
        "/app/files/list/docs?path=notes",
        None,
    )
    .await;
    assert!(list_html.contains("todo.txt"));

    let open_html = dispatch(
        &ctx,
        "GET",
        "/app/files/open/docs",
        "/app/files/open/docs?path=notes/todo.txt",
        None,
    )
    .await;
    assert!(open_html.contains("updated content"));

    let download_payload = dispatch(
        &ctx,
        "GET",
        "/app/files/download/docs",
        "/app/files/download/docs?path=notes/todo.txt",
        None,
    )
    .await;
    assert!(download_payload.contains("\"__sushi_file_download\":true"));

    let frontend_script = std::fs::read_to_string(plugin_root.join("web/static/file_browser.js"))
        .expect("read web script");
    assert!(
        frontend_script.contains("toggle-dir"),
        "file browser frontend should support directory toggle action"
    );
    assert!(
        frontend_script.contains("data-fb-children-for"),
        "file browser frontend should support lazy tree children containers"
    );
    assert!(
        frontend_script.contains("scrollIntoView"),
        "file browser frontend should auto-scroll selected nodes into view"
    );
    assert!(
        frontend_script.contains("ctx-create-dir"),
        "file browser frontend should handle explorer context-menu actions"
    );
    assert!(
        frontend_script.contains("ctx-upload"),
        "file browser frontend should expose context-menu upload action"
    );
    assert!(
        frontend_script.contains("toggle-search"),
        "file browser frontend should expose left-rail search action"
    );
    assert!(
        frontend_script.contains("runSearchNow"),
        "file browser frontend should support recursive search"
    );
    assert!(
        frontend_script.contains("query.set(\"root\", this.rootId)"),
        "root switch should include selected root in query"
    );
    assert!(
        !frontend_script.contains("path: this.relPath || \"\""),
        "root switch should not carry previous root path into another root"
    );
    assert!(
        frontend_script.contains("select-dir"),
        "file browser frontend should support row-click directory selection action"
    );
    assert!(
        frontend_script.contains("rotate-90"),
        "file browser frontend should rotate chevrons instead of text glyph swapping"
    );

    let saved =
        std::fs::read_to_string(docs_root.join("notes").join("todo.txt")).expect("read saved file");
    assert_eq!(saved, "updated content");
}
