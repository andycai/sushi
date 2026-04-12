use minijinja::Error as MinijinjaError;
use std::io;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TemplateError {
    #[error("template root missing: {path}")]
    RootMissing { path: String },
    #[error("template root is not a directory: {path}")]
    RootNotDirectory { path: String },
    #[error("template root access error ({path}): {source}")]
    RootAccess {
        path: String,
        #[source]
        source: io::Error,
    },
    #[error("failed to load template {path}: {source}")]
    TemplateLoad {
        path: String,
        #[source]
        source: MinijinjaError,
    },
    #[error("failed to render template: {source}")]
    Render {
        #[source]
        source: MinijinjaError,
    },
}
