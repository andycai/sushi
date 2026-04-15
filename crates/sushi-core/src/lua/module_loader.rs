use mlua::{Lua, Value};
use std::path::{Component, Path, PathBuf};

fn validate_module_name(module: &str) -> Result<(), mlua::Error> {
    let trimmed = module.trim();
    if trimmed.is_empty() {
        return Err(mlua::Error::RuntimeError(
            "unsafe module path: empty name".to_string(),
        ));
    }

    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") || trimmed.starts_with("//") {
        return Err(mlua::Error::RuntimeError(format!(
            "unsafe module path: {trimmed}"
        )));
    }

    if trimmed.starts_with('.') || trimmed.ends_with('.') || trimmed.contains("..") {
        return Err(mlua::Error::RuntimeError(format!(
            "unsafe module path: {trimmed}"
        )));
    }

    if trimmed.contains('/') || trimmed.contains('\\') || trimmed.contains(':') {
        return Err(mlua::Error::RuntimeError(format!(
            "unsafe module path: {trimmed}"
        )));
    }

    if !trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '.')
    {
        return Err(mlua::Error::RuntimeError(format!(
            "unsafe module path: {trimmed}"
        )));
    }

    Ok(())
}

fn safe_module_join(root: &Path, relative: &Path) -> Option<PathBuf> {
    if relative.is_absolute() {
        return None;
    }

    let mut out = root.to_path_buf();
    for component in relative.components() {
        match component {
            Component::Normal(segment) => out.push(segment),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(out)
}

pub fn install_plugin_require(lua: &Lua, plugin_root: &Path) -> Result<(), mlua::Error> {
    let modules_root = plugin_root.join("lua");
    let cache = lua.create_table()?;

    let require_fn = lua.create_function(move |lua, module: String| {
        validate_module_name(&module)?;

        let cached = cache.get::<Value>(module.as_str()).unwrap_or(Value::Nil);
        if !matches!(cached, Value::Nil) {
            return Ok(cached);
        }

        let relative = PathBuf::from(module.replace('.', "/") + ".lua");
        let module_path = safe_module_join(&modules_root, &relative).ok_or_else(|| {
            mlua::Error::RuntimeError(format!("unsafe module path: {module}"))
        })?;

        let source = std::fs::read_to_string(&module_path).map_err(|err| {
            mlua::Error::RuntimeError(format!("read module {} failed: {err}", module_path.display()))
        })?;

        let chunk_name = format!("@{}", module_path.display());
        let loaded = lua
            .load(&source)
            .set_name(&chunk_name)
            .eval::<Value>()
            .map_err(|err| {
                mlua::Error::RuntimeError(format!("load module {module} failed: {err}"))
            })?;

        let normalized = if matches!(loaded, Value::Nil) {
            Value::Boolean(true)
        } else {
            loaded
        };
        cache.set(module.as_str(), normalized.clone())?;
        Ok(normalized)
    })?;

    lua.globals().set("require", require_fn)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::install_plugin_require;
    use crate::lua::vm::create_sandboxed_vm;

    #[test]
    fn require_loads_plugin_local_module() {
        let tmp = tempfile::tempdir().unwrap();
        let plugin_root = tmp.path().join("official").join("kv-store");
        std::fs::create_dir_all(plugin_root.join("lua/domain")).unwrap();
        std::fs::write(
            plugin_root.join("lua/domain/store.lua"),
            "return { ping = function() return 'ok' end }",
        )
        .unwrap();

        let lua = create_sandboxed_vm().unwrap();
        install_plugin_require(&lua, &plugin_root).unwrap();

        let value: String = lua
            .load("local m = require('domain.store'); return m.ping()")
            .eval()
            .unwrap();
        assert_eq!(value, "ok");
    }

    #[test]
    fn require_rejects_parent_traversal() {
        let tmp = tempfile::tempdir().unwrap();
        let plugin_root = tmp.path().join("official").join("kv-store");
        std::fs::create_dir_all(plugin_root.join("lua")).unwrap();

        let lua = create_sandboxed_vm().unwrap();
        install_plugin_require(&lua, &plugin_root).unwrap();

        let err = lua.load("return require('../secrets')").exec().unwrap_err();
        assert!(err.to_string().contains("unsafe module path"));
    }
}
