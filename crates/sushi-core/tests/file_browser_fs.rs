use std::path::Path;

use sushi_core::fs::{FileBrowserFsService, FsError};
use sushi_core::plugin::{
    PluginFileBrowserCapabilities, PluginFileBrowserConfig, PluginFileBrowserRoot,
};

fn config_for(root: &Path, capabilities: PluginFileBrowserCapabilities) -> PluginFileBrowserConfig {
    PluginFileBrowserConfig {
        route_prefix: "/app/files".to_string(),
        hide_dotfiles: true,
        deny_symlink: true,
        text_extensions: Vec::new(),
        roots: vec![PluginFileBrowserRoot {
            id: "docs".to_string(),
            title: "Documents".to_string(),
            path: root.to_string_lossy().to_string(),
            capabilities,
        }],
    }
}

#[tokio::test]
async fn list_rejects_parent_directory_escape() {
    let tmp = tempfile::tempdir().expect("create tempdir");
    let cfg = config_for(
        tmp.path(),
        PluginFileBrowserCapabilities {
            can_list: true,
            can_view_text: true,
            can_edit_text: true,
            can_create_text: true,
            can_create_dir: true,
            can_rename: true,
            can_delete: true,
            can_upload: true,
            can_download: true,
        },
    );
    let service = FileBrowserFsService::from_manifest(&cfg).expect("service should build");

    let err = service
        .list("docs", "../etc")
        .await
        .expect_err("parent escape should fail");
    assert!(matches!(err, FsError::InvalidPath(_)));
}

#[tokio::test]
async fn read_text_rejects_non_whitelisted_extension() {
    let tmp = tempfile::tempdir().expect("create tempdir");
    std::fs::write(tmp.path().join("archive.bin"), [1, 2, 3]).expect("write binary fixture");

    let cfg = config_for(
        tmp.path(),
        PluginFileBrowserCapabilities {
            can_list: true,
            can_view_text: true,
            can_edit_text: true,
            can_create_text: true,
            can_create_dir: true,
            can_rename: true,
            can_delete: true,
            can_upload: true,
            can_download: true,
        },
    );
    let service = FileBrowserFsService::from_manifest(&cfg).expect("service should build");

    let err = service
        .read_text("docs", "archive.bin")
        .await
        .expect_err("binary extension should be rejected");
    assert_eq!(err, FsError::NotTextFile);
}

#[tokio::test]
async fn read_text_rejects_symlink_target() {
    let tmp = tempfile::tempdir().expect("create tempdir");
    let target = tmp.path().join("target.txt");
    let link = tmp.path().join("link.txt");
    std::fs::write(&target, "hello").expect("write text fixture");
    create_symlink(&target, &link);

    let cfg = config_for(
        tmp.path(),
        PluginFileBrowserCapabilities {
            can_list: true,
            can_view_text: true,
            can_edit_text: true,
            can_create_text: true,
            can_create_dir: true,
            can_rename: true,
            can_delete: true,
            can_upload: true,
            can_download: true,
        },
    );
    let service = FileBrowserFsService::from_manifest(&cfg).expect("service should build");

    let err = service
        .read_text("docs", "link.txt")
        .await
        .expect_err("symlink should be rejected");
    assert_eq!(err, FsError::ForbiddenSymlink);
}

#[tokio::test]
async fn write_delete_operations_require_capabilities() {
    let tmp = tempfile::tempdir().expect("create tempdir");
    std::fs::write(tmp.path().join("note.txt"), "hello").expect("write text fixture");

    let cfg = config_for(
        tmp.path(),
        PluginFileBrowserCapabilities {
            can_list: true,
            can_view_text: true,
            can_edit_text: false,
            can_create_text: false,
            can_create_dir: false,
            can_rename: false,
            can_delete: false,
            can_upload: false,
            can_download: false,
        },
    );
    let service = FileBrowserFsService::from_manifest(&cfg).expect("service should build");

    let write_err = service
        .write_text("docs", "note.txt", "updated")
        .await
        .expect_err("write without capability should fail");
    assert!(matches!(write_err, FsError::PermissionDenied(_)));

    let delete_err = service
        .delete("docs", "note.txt")
        .await
        .expect_err("delete without capability should fail");
    assert!(matches!(delete_err, FsError::PermissionDenied(_)));
}

#[tokio::test]
async fn rename_rejects_existing_destination() {
    let tmp = tempfile::tempdir().expect("create tempdir");
    std::fs::write(tmp.path().join("from.txt"), "from").expect("write source fixture");
    std::fs::write(tmp.path().join("to.txt"), "to").expect("write destination fixture");

    let cfg = config_for(
        tmp.path(),
        PluginFileBrowserCapabilities {
            can_list: true,
            can_view_text: true,
            can_edit_text: true,
            can_create_text: true,
            can_create_dir: true,
            can_rename: true,
            can_delete: true,
            can_upload: true,
            can_download: true,
        },
    );
    let service = FileBrowserFsService::from_manifest(&cfg).expect("service should build");

    let err = service
        .rename("docs", "from.txt", "to.txt")
        .await
        .expect_err("rename should not overwrite existing destination");
    assert_eq!(err, FsError::Conflict);
}

#[tokio::test]
async fn rename_directory_moves_nested_entries() {
    let tmp = tempfile::tempdir().expect("create tempdir");
    let from_dir = tmp.path().join("from-dir");
    std::fs::create_dir(&from_dir).expect("create source directory");
    std::fs::write(from_dir.join("note.txt"), "hello").expect("write source nested file");

    let cfg = config_for(
        tmp.path(),
        PluginFileBrowserCapabilities {
            can_list: true,
            can_view_text: true,
            can_edit_text: true,
            can_create_text: true,
            can_create_dir: true,
            can_rename: true,
            can_delete: true,
            can_upload: true,
            can_download: true,
        },
    );
    let service = FileBrowserFsService::from_manifest(&cfg).expect("service should build");

    service
        .rename("docs", "from-dir", "to-dir")
        .await
        .expect("directory rename should succeed");

    assert!(!tmp.path().join("from-dir").exists());
    assert_eq!(
        std::fs::read_to_string(tmp.path().join("to-dir").join("note.txt"))
            .expect("read moved nested file"),
        "hello"
    );
}

#[tokio::test]
async fn rename_rejects_directory_target_inside_source() {
    let tmp = tempfile::tempdir().expect("create tempdir");
    let from_dir = tmp.path().join("from-dir");
    std::fs::create_dir(&from_dir).expect("create source directory");
    std::fs::write(from_dir.join("note.txt"), "hello").expect("write source nested file");

    let cfg = config_for(
        tmp.path(),
        PluginFileBrowserCapabilities {
            can_list: true,
            can_view_text: true,
            can_edit_text: true,
            can_create_text: true,
            can_create_dir: true,
            can_rename: true,
            can_delete: true,
            can_upload: true,
            can_download: true,
        },
    );
    let service = FileBrowserFsService::from_manifest(&cfg).expect("service should build");

    let err = service
        .rename("docs", "from-dir", "from-dir/child")
        .await
        .expect_err("directory rename into child should be rejected");
    assert!(matches!(err, FsError::InvalidPath(_)));
}

#[tokio::test]
async fn write_upload_rejects_existing_destination() {
    let tmp = tempfile::tempdir().expect("create tempdir");
    std::fs::write(tmp.path().join("upload.bin"), [1, 2, 3]).expect("write destination fixture");

    let cfg = config_for(
        tmp.path(),
        PluginFileBrowserCapabilities {
            can_list: true,
            can_view_text: true,
            can_edit_text: true,
            can_create_text: true,
            can_create_dir: true,
            can_rename: true,
            can_delete: true,
            can_upload: true,
            can_download: true,
        },
    );
    let service = FileBrowserFsService::from_manifest(&cfg).expect("service should build");

    let err = service
        .write_upload("docs", "upload.bin", &[9, 9, 9])
        .await
        .expect_err("upload should fail when target exists");
    assert_eq!(err, FsError::Conflict);
}

#[test]
fn from_manifest_rejects_duplicate_root_ids() {
    let tmp = tempfile::tempdir().expect("create tempdir");
    let cfg = PluginFileBrowserConfig {
        route_prefix: "/app/files".to_string(),
        roots: vec![
            PluginFileBrowserRoot {
                id: "docs".to_string(),
                title: "Documents".to_string(),
                path: tmp.path().to_string_lossy().to_string(),
                capabilities: PluginFileBrowserCapabilities {
                    can_list: true,
                    can_view_text: true,
                    can_edit_text: true,
                    can_create_text: true,
                    can_create_dir: true,
                    can_rename: true,
                    can_delete: true,
                    can_upload: true,
                    can_download: true,
                },
            },
            PluginFileBrowserRoot {
                id: "docs".to_string(),
                title: "Duplicate".to_string(),
                path: tmp.path().to_string_lossy().to_string(),
                capabilities: PluginFileBrowserCapabilities {
                    can_list: true,
                    can_view_text: true,
                    can_edit_text: true,
                    can_create_text: true,
                    can_create_dir: true,
                    can_rename: true,
                    can_delete: true,
                    can_upload: true,
                    can_download: true,
                },
            },
        ],
        hide_dotfiles: true,
        deny_symlink: true,
        text_extensions: Vec::new(),
    };

    let err =
        FileBrowserFsService::from_manifest(&cfg).expect_err("duplicate roots should be rejected");
    assert!(matches!(err, FsError::InvalidPath(_)));
}

#[tokio::test]
async fn from_manifest_with_root_base_resolves_relative_paths() {
    let tmp = tempfile::tempdir().expect("create tempdir");
    let root_base = tmp.path().join("workspace");
    let docs_dir = root_base.join("docs");
    std::fs::create_dir_all(&docs_dir).expect("create docs dir");
    std::fs::write(docs_dir.join("note.txt"), "hello").expect("write fixture");

    let cfg = PluginFileBrowserConfig {
        route_prefix: "/app/files".to_string(),
        hide_dotfiles: true,
        deny_symlink: true,
        text_extensions: Vec::new(),
        roots: vec![PluginFileBrowserRoot {
            id: "docs".to_string(),
            title: "Documents".to_string(),
            path: "docs".to_string(),
            capabilities: PluginFileBrowserCapabilities {
                can_list: true,
                can_view_text: true,
                can_edit_text: true,
                can_create_text: true,
                can_create_dir: true,
                can_rename: true,
                can_delete: true,
                can_upload: true,
                can_download: true,
            },
        }],
    };

    let service = FileBrowserFsService::from_manifest_with_root_base(&cfg, &root_base)
        .expect("service should build");
    let entries = service.list("docs", "").await.expect("list root entries");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "note.txt");
}

#[cfg(unix)]
fn create_symlink(target: &Path, link: &Path) {
    std::os::unix::fs::symlink(target, link).expect("create unix symlink");
}

#[cfg(windows)]
fn create_symlink(target: &Path, link: &Path) {
    std::os::windows::fs::symlink_file(target, link).expect("create windows symlink");
}
