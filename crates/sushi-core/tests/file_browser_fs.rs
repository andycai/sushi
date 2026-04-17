use std::path::Path;

use sushi_core::fs::{FileBrowserFsService, FsError};
use sushi_core::plugin::{
    PluginFileBrowserCapabilities, PluginFileBrowserConfig, PluginFileBrowserRoot,
};

fn config_for(root: &Path, capabilities: PluginFileBrowserCapabilities) -> PluginFileBrowserConfig {
    PluginFileBrowserConfig {
        route_prefix: "/app/files".to_string(),
        roots: vec![PluginFileBrowserRoot {
            id: "docs".to_string(),
            path: root.to_string_lossy().to_string(),
        }],
        capabilities,
    }
}

#[tokio::test]
async fn list_rejects_parent_directory_escape() {
    let tmp = tempfile::tempdir().expect("create tempdir");
    let cfg = config_for(
        tmp.path(),
        PluginFileBrowserCapabilities {
            read: true,
            write: true,
            delete: true,
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
            read: true,
            write: true,
            delete: true,
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
            read: true,
            write: true,
            delete: true,
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
            read: true,
            write: false,
            delete: false,
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
            read: true,
            write: true,
            delete: true,
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
async fn write_upload_rejects_existing_destination() {
    let tmp = tempfile::tempdir().expect("create tempdir");
    std::fs::write(tmp.path().join("upload.bin"), [1, 2, 3]).expect("write destination fixture");

    let cfg = config_for(
        tmp.path(),
        PluginFileBrowserCapabilities {
            read: true,
            write: true,
            delete: true,
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
                path: tmp.path().to_string_lossy().to_string(),
            },
            PluginFileBrowserRoot {
                id: "docs".to_string(),
                path: tmp.path().to_string_lossy().to_string(),
            },
        ],
        capabilities: PluginFileBrowserCapabilities {
            read: true,
            write: true,
            delete: true,
        },
    };

    let err = FileBrowserFsService::from_manifest(&cfg)
        .expect_err("duplicate roots should be rejected");
    assert!(matches!(err, FsError::InvalidPath(_)));
}

#[cfg(unix)]
fn create_symlink(target: &Path, link: &Path) {
    std::os::unix::fs::symlink(target, link).expect("create unix symlink");
}

#[cfg(windows)]
fn create_symlink(target: &Path, link: &Path) {
    std::os::windows::fs::symlink_file(target, link).expect("create windows symlink");
}
