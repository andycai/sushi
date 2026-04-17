use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::plugin::{PluginFileBrowserCapabilities, PluginFileBrowserConfig};

#[derive(Debug, Clone)]
pub struct FileBrowserFsService {
    roots: HashMap<String, FsRoot>,
    route_prefix: String,
    hide_dotfiles: bool,
    deny_symlink: bool,
    text_extensions: HashSet<String>,
}

#[derive(Debug, Clone)]
struct FsRoot {
    id: String,
    title: String,
    path: PathBuf,
    capabilities: PluginFileBrowserCapabilities,
}

#[derive(Debug, Clone, Copy)]
enum RequiredCapability {
    List,
    ViewText,
    EditText,
    CreateText,
    CreateDir,
    Rename,
    Delete,
    Upload,
    Download,
}

impl RequiredCapability {
    fn denied_flag(self) -> &'static str {
        match self {
            Self::List => "can_list",
            Self::ViewText => "can_view_text",
            Self::EditText => "can_edit_text",
            Self::CreateText => "can_create_text",
            Self::CreateDir => "can_create_dir",
            Self::Rename => "can_rename",
            Self::Delete => "can_delete",
            Self::Upload => "can_upload",
            Self::Download => "can_download",
        }
    }
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadPayload {
    pub ticket: DownloadTicket,
    pub content: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FsRootDescriptor {
    pub id: String,
    pub title: String,
    pub path: String,
    pub capabilities: PluginFileBrowserCapabilities,
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
            if roots.contains_key(&root.id) {
                return Err(FsError::InvalidPath(format!(
                    "duplicate root id '{}'",
                    root.id
                )));
            }
            let path = std::fs::canonicalize(&root.path).map_err(map_io_error)?;
            roots.insert(
                root.id.clone(),
                FsRoot {
                    id: root.id.clone(),
                    title: if root.title.trim().is_empty() {
                        root.id.clone()
                    } else {
                        root.title.trim().to_string()
                    },
                    path,
                    capabilities: root.capabilities.clone(),
                },
            );
        }

        let text_extensions = normalized_text_extensions(config);

        Ok(Self {
            roots,
            route_prefix: config.route_prefix.clone(),
            hide_dotfiles: config.hide_dotfiles,
            deny_symlink: config.deny_symlink,
            text_extensions,
        })
    }

    pub fn route_prefix(&self) -> &str {
        &self.route_prefix
    }

    pub fn roots(&self) -> Vec<FsRootDescriptor> {
        let mut roots = self
            .roots
            .values()
            .map(|root| FsRootDescriptor {
                id: root.id.clone(),
                title: root.title.clone(),
                path: root.path.to_string_lossy().to_string(),
                capabilities: root.capabilities.clone(),
            })
            .collect::<Vec<_>>();
        roots.sort_by(|left, right| left.id.cmp(&right.id));
        roots
    }

    pub async fn list(&self, root_id: &str, rel_path: &str) -> Result<Vec<FsEntry>, FsError> {
        let root = self.root(root_id, RequiredCapability::List)?;
        let target = self.resolve_existing(root, rel_path, true)?;

        let mut read_dir = tokio::fs::read_dir(&target).await.map_err(map_io_error)?;
        let mut out = Vec::new();

        while let Some(entry) = read_dir.next_entry().await.map_err(map_io_error)? {
            let file_name = entry.file_name().to_string_lossy().to_string();
            if self.hide_dotfiles && is_hidden_segment(&file_name) {
                continue;
            }
            let metadata = tokio::fs::symlink_metadata(entry.path())
                .await
                .map_err(map_io_error)?;
            if self.deny_symlink && metadata.file_type().is_symlink() {
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
        let root = self.root(root_id, RequiredCapability::ViewText)?;
        let target = self.resolve_existing(root, rel_path, false)?;
        ensure_text_extension(&target, &self.text_extensions)?;
        tokio::fs::read_to_string(target)
            .await
            .map_err(map_io_error)
    }

    pub async fn write_text(
        &self,
        root_id: &str,
        rel_path: &str,
        content: &str,
    ) -> Result<(), FsError> {
        let root = self.root(root_id, RequiredCapability::EditText)?;
        let target = self.resolve_existing(root, rel_path, false)?;
        ensure_text_extension(&target, &self.text_extensions)?;
        tokio::fs::write(target, content)
            .await
            .map_err(map_io_error)
    }

    pub async fn create_text(
        &self,
        root_id: &str,
        rel_path: &str,
        initial_content: Option<&str>,
    ) -> Result<(), FsError> {
        let root = self.root(root_id, RequiredCapability::CreateText)?;
        let target = self.resolve_for_create(root, rel_path)?;
        ensure_text_extension(&target, &self.text_extensions)?;

        let mut opts = tokio::fs::OpenOptions::new();
        opts.write(true).create_new(true);
        let mut file = opts.open(target).await.map_err(map_io_error)?;

        if let Some(content) = initial_content {
            use tokio::io::AsyncWriteExt;
            file.write_all(content.as_bytes())
                .await
                .map_err(map_io_error)?;
        }

        Ok(())
    }

    pub async fn mkdir(&self, root_id: &str, rel_path: &str) -> Result<(), FsError> {
        let root = self.root(root_id, RequiredCapability::CreateDir)?;
        let target = self.resolve_for_create(root, rel_path)?;
        tokio::fs::create_dir(target).await.map_err(map_io_error)
    }

    pub async fn rename(
        &self,
        root_id: &str,
        from_rel_path: &str,
        to_rel_path: &str,
    ) -> Result<(), FsError> {
        let root = self.root(root_id, RequiredCapability::Rename)?;
        let from = self.resolve_existing(root, from_rel_path, false)?;
        let to = self.resolve_for_create(root, to_rel_path)?;
        let metadata = tokio::fs::metadata(&from).await.map_err(map_io_error)?;
        if metadata.is_dir() {
            if to.starts_with(&from) {
                return Err(FsError::InvalidPath(
                    "directory rename target cannot be inside source directory".to_string(),
                ));
            }
            return rename_directory_no_overwrite(&from, &to);
        }

        // Use hard link + remove to avoid destination overwrite races.
        tokio::fs::hard_link(&from, &to)
            .await
            .map_err(map_io_error)?;
        tokio::fs::remove_file(from).await.map_err(map_io_error)
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
        let root = self.root(root_id, RequiredCapability::Upload)?;
        let target = self.resolve_for_create(root, rel_path)?;
        let mut opts = tokio::fs::OpenOptions::new();
        opts.write(true).create_new(true);
        let mut file = opts.open(target).await.map_err(map_io_error)?;
        use tokio::io::AsyncWriteExt;
        file.write_all(content).await.map_err(map_io_error)?;
        Ok(())
    }

    pub async fn prepare_download(
        &self,
        root_id: &str,
        rel_path: &str,
    ) -> Result<DownloadTicket, FsError> {
        let root = self.root(root_id, RequiredCapability::Download)?;
        let target = self.resolve_existing(root, rel_path, false)?;
        let metadata = tokio::fs::metadata(&target).await.map_err(map_io_error)?;
        if metadata.is_dir() {
            return Err(FsError::InvalidPath(
                "download target must be a file".to_string(),
            ));
        }

        let file_name = target
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                FsError::InvalidPath("download target has invalid filename".to_string())
            })?
            .to_string();

        Ok(DownloadTicket {
            root_id: root_id.to_string(),
            rel_path: normalize_rel_path(rel_path),
            file_name,
            size: metadata.len(),
        })
    }

    pub async fn read_download(
        &self,
        root_id: &str,
        rel_path: &str,
    ) -> Result<DownloadPayload, FsError> {
        let ticket = self.prepare_download(root_id, rel_path).await?;
        let root = self.root(root_id, RequiredCapability::Download)?;
        let target = self.resolve_existing(root, rel_path, false)?;
        let content = tokio::fs::read(target).await.map_err(map_io_error)?;
        Ok(DownloadPayload { ticket, content })
    }

    fn root(&self, root_id: &str, capability: RequiredCapability) -> Result<&FsRoot, FsError> {
        let root = self
            .roots
            .get(root_id)
            .ok_or_else(|| FsError::RootNotFound(root_id.to_string()))?;

        let allowed = match capability {
            RequiredCapability::List => root.capabilities.can_list,
            RequiredCapability::ViewText => root.capabilities.can_view_text,
            RequiredCapability::EditText => root.capabilities.can_edit_text,
            RequiredCapability::CreateText => root.capabilities.can_create_text,
            RequiredCapability::CreateDir => root.capabilities.can_create_dir,
            RequiredCapability::Rename => root.capabilities.can_rename,
            RequiredCapability::Delete => root.capabilities.can_delete,
            RequiredCapability::Upload => root.capabilities.can_upload,
            RequiredCapability::Download => root.capabilities.can_download,
        };

        if allowed {
            Ok(root)
        } else {
            Err(FsError::PermissionDenied(format!(
                "capability '{}' denied for root '{root_id}'",
                capability.denied_flag()
            )))
        }
    }

    fn resolve_existing(
        &self,
        root: &FsRoot,
        rel_path: &str,
        allow_root: bool,
    ) -> Result<PathBuf, FsError> {
        let segments = parse_rel_segments(rel_path, self.hide_dotfiles)?;
        if segments.is_empty() {
            if allow_root {
                return Ok(root.path.clone());
            }
            return Err(FsError::InvalidPath("path cannot target root".to_string()));
        }

        resolve_segments_existing(&root.path, &segments, self.deny_symlink)
    }

    fn resolve_for_create(&self, root: &FsRoot, rel_path: &str) -> Result<PathBuf, FsError> {
        let segments = parse_rel_segments(rel_path, self.hide_dotfiles)?;
        if segments.is_empty() {
            return Err(FsError::InvalidPath("path cannot target root".to_string()));
        }

        let (parent_segments, leaf) = segments.split_at(segments.len() - 1);
        if leaf[0].is_empty() {
            return Err(FsError::InvalidPath(
                "path cannot end with empty segment".to_string(),
            ));
        }

        let parent = if parent_segments.is_empty() {
            root.path.clone()
        } else {
            resolve_segments_existing(&root.path, parent_segments, self.deny_symlink)?
        };

        Ok(parent.join(&leaf[0]))
    }
}

fn resolve_segments_existing(
    base: &Path,
    segments: &[String],
    deny_symlink: bool,
) -> Result<PathBuf, FsError> {
    let mut current = base.to_path_buf();
    for segment in segments {
        current.push(segment);
        let metadata = std::fs::symlink_metadata(&current).map_err(map_io_error)?;
        if deny_symlink && metadata.file_type().is_symlink() {
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

fn parse_rel_segments(rel_path: &str, hide_dotfiles: bool) -> Result<Vec<String>, FsError> {
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
                if hide_dotfiles && is_hidden_segment(&segment) {
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
        "txt", "md", "markdown", "json", "toml", "yaml", "yml", "ini", "csv", "log", "lua", "rs",
        "js", "jsx", "ts", "tsx", "css", "html", "htm", "xml", "sh",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn normalized_text_extensions(config: &PluginFileBrowserConfig) -> HashSet<String> {
    if config.text_extensions.is_empty() {
        return default_text_extensions();
    }

    let mut out = HashSet::new();
    for ext in &config.text_extensions {
        let normalized = ext.trim().trim_start_matches('.').to_ascii_lowercase();
        if !normalized.is_empty() {
            out.insert(normalized);
        }
    }

    if out.is_empty() {
        default_text_extensions()
    } else {
        out
    }
}

fn rename_directory_no_overwrite(from: &Path, to: &Path) -> Result<(), FsError> {
    std::fs::create_dir(to).map_err(map_io_error)?;
    let read_dir = std::fs::read_dir(from).map_err(map_io_error)?;
    for entry in read_dir {
        let entry = entry.map_err(map_io_error)?;
        let source_path = entry.path();
        let target_path = to.join(entry.file_name());
        let file_type = entry.file_type().map_err(map_io_error)?;
        if file_type.is_dir() {
            rename_directory_no_overwrite(&source_path, &target_path)?;
        } else {
            std::fs::rename(&source_path, &target_path).map_err(map_io_error)?;
        }
    }
    std::fs::remove_dir(from).map_err(map_io_error)
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
