use minijinja::{Environment, Error as MinijinjaError, ErrorKind as MinijinjaErrorKind};
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use super::template_error::TemplateError;

#[derive(Clone)]
pub struct TemplateService {
    env: Arc<Environment<'static>>,
}

impl TemplateService {
    pub fn new(root: &Path) -> Result<Self, TemplateError> {
        Self::new_with_plugin_roots(root, Vec::<(String, PathBuf)>::new())
    }

    pub fn new_with_plugin_roots<I, S>(root: &Path, plugin_roots: I) -> Result<Self, TemplateError>
    where
        I: IntoIterator<Item = (S, PathBuf)>,
        S: Into<String>,
    {
        validate_root(root)?;

        let mut normalized_plugin_roots = HashMap::new();
        for (plugin_name, plugin_root) in plugin_roots {
            let plugin_name = plugin_name.into();
            if plugin_name.trim().is_empty() {
                continue;
            }
            if validate_root_optional(&plugin_root).is_ok() {
                normalized_plugin_roots.insert(plugin_name, plugin_root);
            }
        }

        let mut env: Environment<'static> = Environment::new();
        let main_root = root.to_path_buf();
        let plugin_roots = Arc::new(normalized_plugin_roots);
        env.set_loader(move |name: &str| {
            if let Some((plugin_name, plugin_path)) = split_plugin_template_name(name) {
                if let Some(plugin_root) = plugin_roots.get(plugin_name.as_str()) {
                    if let Some(source) = load_template(plugin_root, plugin_path)? {
                        return Ok(Some(source));
                    }
                }
            }
            load_template(&main_root, name)
        });

        Ok(Self { env: Arc::new(env) })
    }

    pub fn render<C: Serialize>(&self, name: &str, context: C) -> Result<String, TemplateError> {
        let template =
            self.env
                .get_template(name)
                .map_err(|source| TemplateError::TemplateLoad {
                    path: name.to_string(),
                    source,
                })?;
        template
            .render(context)
            .map_err(|source| TemplateError::Render { source })
    }
}

fn validate_root(root: &Path) -> Result<(), TemplateError> {
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

    Ok(())
}

fn validate_root_optional(root: &Path) -> Result<(), TemplateError> {
    if !root.exists() {
        return Err(TemplateError::RootMissing {
            path: root.display().to_string(),
        });
    }
    validate_root(root)
}

fn split_plugin_template_name(name: &str) -> Option<(String, &str)> {
    let rest = name.strip_prefix("plugins/")?;
    let mut segments = rest.splitn(3, '/');
    let tier = segments.next()?;
    let plugin_name = segments.next()?;
    let plugin_path = segments.next()?;
    if tier.is_empty() || plugin_name.is_empty() || plugin_path.is_empty() {
        return None;
    }
    Some((format!("{tier}/{plugin_name}"), plugin_path))
}

fn load_template(root: &Path, template_name: &str) -> Result<Option<String>, MinijinjaError> {
    let template_path = match safe_join(root, template_name) {
        Some(path) => path,
        None => return Ok(None),
    };

    match fs::read_to_string(&template_path) {
        Ok(source) => Ok(Some(source)),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
        Err(err) => Err(MinijinjaError::new(
            MinijinjaErrorKind::InvalidOperation,
            format!("failed to read template {}: {err}", template_path.display()),
        )),
    }
}

fn safe_join(root: &Path, template_name: &str) -> Option<PathBuf> {
    let relative = Path::new(template_name);
    if relative.is_absolute() {
        return None;
    }

    let mut resolved = root.to_path_buf();
    for component in relative.components() {
        match component {
            Component::Normal(segment) => resolved.push(segment),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }

    Some(resolved)
}
