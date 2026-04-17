use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::plugin::{PluginFileBrowserCapabilities, PluginFileBrowserConfig};

#[derive(Debug, Clone)]
pub struct FileBrowserFsService {
    roots: HashMap<String, FsRoot>,
    text_extensions: HashSet<String>,
}

#[derive(Debug, Clone)]
struct FsRoot {
    path: PathBuf,
    capabilities: PluginFileBrowserCapabilities,
}

#[derive(Debug, Clone, Copy)]
enum RequiredCapability {
    Read,
    Write,
    Delete,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FsEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DownloadTicket {
    pub root_id: String,
    pub rel_path: String,
    pub file_name: String,
    pub size: u64,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FsError {
    #[error("invalid_path: {0}")]
    InvalidPath(String),
    #[error("root_not_found: unknown root '{0}'")]
    RootNotFound(String),
    #[error("permission_denied: {0}")]
    PermissionDenied(String),
    #[error("forbidden_hidden: hidden paths are not allowed")]
    ForbiddenHidden,
    #[error("forbidden_symlink: symlinks are not allowed")]
    ForbiddenSymlink,
    #[error("not_text_file: only configured text extensions are allowed")]
    NotTextFile,
    #[error("not_found: target does not exist")]
    NotFound,
    #[error("conflict: target already exists")]
    Conflict,
    #[error("not_empty_dir: directory is not empty")]
    NotEmptyDir,
    #[error("io_error: {0}")]
    IoError(String),
}

impl FsError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidPath(_) => "invalid_path",
            Self::RootNotFound(_) => "root_not_found",
            Self::PermissionDenied(_) => "permission_denied",
            Self::ForbiddenHidden => "forbidden_hidden",
            Self::ForbiddenSymlink => "forbidden_symlink",
            Self::NotTextFile => "not_text_file",
            Self::NotFound => "not_found",
            Self::Conflict => "conflict",
            Self::NotEmptyDir => "not_empty_dir",
            Self::IoError(_) => "io_error",
        }
    }
}

impl From<std::io::Error> for FsError {
    fn from(err: std::io::Error) -> Self {
        map_io_error(err)
    }
}

impl FileBrowserFsService {
    pub fn from_manifest(config: &PluginFileBrowserConfig) -> Result<Self, FsError> {
        let mut roots = HashMap::with_capacity(config.roots.len());
        for root in &config.roots {
            let path = std::fs::canonicalize(&root.path).map_err(map_io_error)?;
            roots.insert(
                root.id.clone(),
                FsRoot {
                    path,
                    capabilities: config.capabilities.clone(),
                },
            );
        }

        Ok(Self {
            roots,
            text_extensions: default_text_extensions(),
        })
    }

    pub async fn list(&self, root_id: &str, rel_path: &str) -> Result<Vec<FsEntry>, FsError> {
        let root = self.root(root_id, RequiredCapability::Read)?;
        let target = self.resolve_existing(root, rel_path, true)?;

        let mut read_dir = tokio::fs::read_dir(&target).await.map_err(map_io_error)?;
        let mut out = Vec::new();

        while let Some(entry) = read_dir.next_entry().await.map_err(map_io_error)? {
            let file_name = entry.file_name().to_string_lossy().to_string();
            if is_hidden_segment(&file_name) {
                continue;
            }
            let metadata = tokio::fs::symlink_metadata(entry.path())
                .await
                .map_err(map_io_error)?;
            if metadata.file_type().is_symlink() {
                continue;
            }

            let rel = normalize_rel_path(rel_path);
            let child_rel = if rel.is_empty() {
                file_name.clone()
            } else {
                format!("{rel}/{file_name}")
            };

            out.push(FsEntry {
                name: file_name,
                path: child_rel,
                is_dir: metadata.is_dir(),
                size: metadata.len(),
            });
        }

        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    pub async fn read_text(&self, root_id: &str, rel_path: &str) -> Result<String, FsError> {
        let root = self.root(root_id, RequiredCapability::Read)?;
        let target = self.resolve_existing(root, rel_path, false)?;
        ensure_text_extension(&target, &self.text_extensions)?;
        tokio::fs::read_to_string(target).await.map_err(map_io_error)
    }

    pub async fn write_text(
        &self,
        root_id: &str,
        rel_path: &str,
        content: &str,
    ) -> Result<(), FsError> {
        let root = self.root(root_id, RequiredCapability::Write)?;
        let target = self.resolve_existing(root, rel_path, false)?;
        ensure_text_extension(&target, &self.text_extensions)?;
        tokio::fs::write(target, content).await.map_err(map_io_error)
    }

    pub async fn create_text(
        &self,
        root_id: &str,
        rel_path: &str,
        initial_content: Option<&str>,
    ) -> Result<(), FsError> {
        let root = self.root(root_id, RequiredCapability::Write)?;
        let target = self.resolve_for_create(root, rel_path)?;
        ensure_text_extension(&target, &self.text_extensions)?;

        let mut opts = tokio::fs::OpenOptions::new();
        opts.write(true).create_new(true);
        let mut file = opts.open(target).await.map_err(map_io_error)?;

        if let Some(content) = initial_content {
            use tokio::io::AsyncWriteExt;
            file.write_all(content.as_bytes()).await.map_err(map_io_error)?;
        }

        Ok(())
    }

    pub async fn mkdir(&self, root_id: &str, rel_path: &str) -> Result<(), FsError> {
        let root = self.root(root_id, RequiredCapability::Write)?;
        let target = self.resolve_for_create(root, rel_path)?;
        tokio::fs::create_dir(target).await.map_err(map_io_error)
    }

    pub async fn rename(&self, root_id: &str, from_rel_path: &str, to_rel_path: &str) -> Result<(), FsError> {
        let root = self.root(root_id, RequiredCapability::Write)?;
        let from = self.resolve_existing(root, from_rel_path, false)?;
        let to = self.resolve_for_create(root, to_rel_path)?;
        tokio::fs::rename(from, to).await.map_err(map_io_error)
    }

    pub async fn delete(&self, root_id: &str, rel_path: &str) -> Result<(), FsError> {
        let root = self.root(root_id, RequiredCapability::Delete)?;
        let target = self.resolve_existing(root, rel_path, false)?;
        let metadata = tokio::fs::metadata(&target).await.map_err(map_io_error)?;
        if metadata.is_dir() {
            tokio::fs::remove_dir(target).await.map_err(map_io_error)
        } else {
            tokio::fs::remove_file(target).await.map_err(map_io_error)
        }
    }

    pub async fn write_upload(
        &self,
        root_id: &str,
        rel_path: &str,
        content: &[u8],
    ) -> Result<(), FsError> {
        let root = self.root(root_id, RequiredCapability::Write)?;
        let target = self.resolve_for_create(root, rel_path)?;
        tokio::fs::write(target, content).await.map_err(map_io_error)
    }

    pub async fn prepare_download(
        &self,
        root_id: &str,
        rel_path: &str,
    ) -> Result<DownloadTicket, FsError> {
        let root = self.root(root_id, RequiredCapability::Read)?;
        let target = self.resolve_existing(root, rel_path, false)?;
        let metadata = tokio::fs::metadata(&target).await.map_err(map_io_error)?;
        if metadata.is_dir() {
            return Err(FsError::InvalidPath("download target must be a file".to_string()));
        }

        let file_name = target
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| FsError::InvalidPath("download target has invalid filename".to_string()))?
            .to_string();

        Ok(DownloadTicket {
            root_id: root_id.to_string(),
            rel_path: normalize_rel_path(rel_path),
            file_name,
            size: metadata.len(),
        })
    }

    fn root(&self, root_id: &str, capability: RequiredCapability) -> Result<&FsRoot, FsError> {
        let root = self
            .roots
            .get(root_id)
            .ok_or_else(|| FsError::RootNotFound(root_id.to_string()))?;

        let allowed = match capability {
            RequiredCapability::Read => root.capabilities.read,
            RequiredCapability::Write => root.capabilities.write,
            RequiredCapability::Delete => root.capabilities.delete,
        };

        if allowed {
            Ok(root)
        } else {
            Err(FsError::PermissionDenied(format!(
                "capability '{:?}' denied for root '{root_id}'",
                capability
            )))
        }
    }

    fn resolve_existing(
        &self,
        root: &FsRoot,
        rel_path: &str,
        allow_root: bool,
    ) -> Result<PathBuf, FsError> {
        let segments = parse_rel_segments(rel_path)?;
        if segments.is_empty() {
            if allow_root {
                return Ok(root.path.clone());
            }
            return Err(FsError::InvalidPath("path cannot target root".to_string()));
        }

        resolve_segments_existing(&root.path, &segments)
    }

    fn resolve_for_create(&self, root: &FsRoot, rel_path: &str) -> Result<PathBuf, FsError> {
        let segments = parse_rel_segments(rel_path)?;
        if segments.is_empty() {
            return Err(FsError::InvalidPath("path cannot target root".to_string()));
        }

        let (parent_segments, leaf) = segments.split_at(segments.len() - 1);
        if leaf[0].is_empty() {
            return Err(FsError::InvalidPath("path cannot end with empty segment".to_string()));
        }

        let parent = if parent_segments.is_empty() {
            root.path.clone()
        } else {
            resolve_segments_existing(&root.path, parent_segments)?
        };

        Ok(parent.join(&leaf[0]))
    }
}

fn resolve_segments_existing(base: &Path, segments: &[String]) -> Result<PathBuf, FsError> {
    let mut current = base.to_path_buf();
    for segment in segments {
        current.push(segment);
        let metadata = std::fs::symlink_metadata(&current).map_err(map_io_error)?;
        if metadata.file_type().is_symlink() {
            return Err(FsError::ForbiddenSymlink);
        }
    }
    Ok(current)
}

fn normalize_rel_path(raw: &str) -> String {
    let trimmed = raw.trim_matches('/').trim();
    if trimmed == "." {
        String::new()
    } else {
        trimmed.to_string()
    }
}

fn parse_rel_segments(rel_path: &str) -> Result<Vec<String>, FsError> {
    let normalized = normalize_rel_path(rel_path);
    if normalized.is_empty() {
        return Ok(Vec::new());
    }

    let path = Path::new(&normalized);
    if path.is_absolute() {
        return Err(FsError::InvalidPath(
            "absolute paths are not allowed".to_string(),
        ));
    }

    let mut segments = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(name) => {
                let segment = name.to_string_lossy().to_string();
                if segment.is_empty() {
                    return Err(FsError::InvalidPath("empty path segment".to_string()));
                }
                if is_hidden_segment(&segment) {
                    return Err(FsError::ForbiddenHidden);
                }
                segments.push(segment);
            }
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(FsError::InvalidPath(
                    "parent traversal is not allowed".to_string(),
                ))
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(FsError::InvalidPath(
                    "absolute paths are not allowed".to_string(),
                ))
            }
        }
    }

    Ok(segments)
}

fn is_hidden_segment(segment: &str) -> bool {
    segment.starts_with('.')
}

fn ensure_text_extension(path: &Path, allowed: &HashSet<String>) -> Result<(), FsError> {
    let ext = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .ok_or(FsError::NotTextFile)?;

    if allowed.contains(&ext) {
        Ok(())
    } else {
        Err(FsError::NotTextFile)
    }
}

fn default_text_extensions() -> HashSet<String> {
    [
        "txt", "md", "markdown", "json", "toml", "yaml", "yml", "ini", "csv", "log", "lua",
        "rs", "js", "jsx", "ts", "tsx", "css", "html", "htm", "xml", "sh",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn map_io_error(err: std::io::Error) -> FsError {
    use std::io::ErrorKind;

    match err.kind() {
        ErrorKind::NotFound => FsError::NotFound,
        ErrorKind::AlreadyExists => FsError::Conflict,
        ErrorKind::DirectoryNotEmpty => FsError::NotEmptyDir,
        ErrorKind::PermissionDenied => {
            FsError::PermissionDenied("filesystem denied operation".to_string())
        }
        _ => FsError::IoError(err.to_string()),
    }
}
