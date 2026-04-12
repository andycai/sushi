use minijinja::{Environment, path_loader};
use serde::Serialize;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::sync::Arc;

use super::template_error::TemplateError;

#[derive(Clone)]
pub struct TemplateService {
    env: Arc<Environment<'static>>,
}

impl TemplateService {
    pub fn new(root: &Path) -> Result<Self, TemplateError> {
        let metadata = fs::metadata(root).map_err(|err| {
            let path = root.display().to_string();
            if err.kind() == ErrorKind::NotFound {
                TemplateError::RootMissing { path }
            } else {
                TemplateError::RootAccess { path, source: err }
            }
        })?;

        if !metadata.is_dir() {
            return Err(TemplateError::RootNotDirectory {
                path: root.display().to_string(),
            });
        }

        let mut env: Environment<'static> = Environment::new();
        env.set_loader(path_loader(root.to_path_buf()));

        Ok(Self {
            env: Arc::new(env),
        })
    }

    pub fn render<C: Serialize>(&self, name: &str, context: C) -> Result<String, TemplateError> {
        let template = self.env.get_template(name).map_err(|source| {
            TemplateError::TemplateLoad {
                path: name.to_string(),
                source,
            }
        })?;
        template.render(context).map_err(|source| TemplateError::Render { source })
    }
}
