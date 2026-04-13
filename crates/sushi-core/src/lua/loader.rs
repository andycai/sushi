use crate::context::SushiContext;
use crate::lua::bindings::inject_sushi_api;
use crate::lua::vm::create_sandboxed_vm;
use crate::plugin::{Plugin, PluginError, PluginManifest};
use async_trait::async_trait;
use std::path::{Path, PathBuf};

/// A Lua-based plugin loaded from the filesystem.
pub struct LuaPlugin {
    manifest: PluginManifest,
    lua: Option<mlua::Lua>,
    plugin_dir: PathBuf,
}

impl LuaPlugin {
    /// Scan a directory for plugins. Returns one LuaPlugin per subdirectory with a plugin.toml.
    pub async fn scan_dir(dir: &Path) -> Result<Vec<Self>, PluginError> {
        let mut plugins = Vec::new();
        let mut entries = tokio::fs::read_dir(dir)
            .await
            .map_err(PluginError::IoError)?;

        while let Some(entry) = entries.next_entry().await.map_err(PluginError::IoError)? {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let manifest_path = path.join("plugin.toml");
            if !manifest_path.exists() {
                continue;
            }

            let manifest_content =
                tokio::fs::read_to_string(&manifest_path)
                    .await
                    .map_err(|e| {
                        PluginError::ManifestError(format!("read {}: {e}", manifest_path.display()))
                    })?;
            let manifest: PluginManifest = toml::from_str(&manifest_content).map_err(|e| {
                PluginError::ManifestError(format!("parse {}: {e}", manifest_path.display()))
            })?;

            let lua = create_sandboxed_vm().map_err(|e| {
                PluginError::LuaError(format!("create VM for {}: {e}", manifest.plugin.name))
            })?;

            plugins.push(Self {
                manifest,
                lua: Some(lua),
                plugin_dir: path,
            });
        }

        Ok(plugins)
    }

    pub fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    /// Take the Lua VM out of the plugin after init.
    /// This transfers ownership to the caller (typically PluginManager).
    pub fn into_vm(self) -> Option<mlua::Lua> {
        self.lua
    }
}

#[async_trait]
impl Plugin for LuaPlugin {
    fn name(&self) -> &str {
        &self.manifest.plugin.name
    }
    fn version(&self) -> &str {
        &self.manifest.plugin.version
    }

    async fn init(&self, ctx: &SushiContext) -> Result<(), PluginError> {
        // Take the Lua VM out of self (init should only be called once)
        let lua = self.lua.as_ref().ok_or_else(|| {
            PluginError::InitFailed(format!(
                "{}: already initialized",
                self.manifest.plugin.name
            ))
        })?;

        // Inject sushi.* API into the Lua VM
        inject_sushi_api(lua, ctx, &self.manifest.permissions)
            .await
            .map_err(|e| PluginError::LuaError(format!("inject API: {e}")))?;

        // Load and execute the entry script
        let entry_path = self.plugin_dir.join(&self.manifest.plugin.entry);

        // Check file size limit (1MB max)
        const MAX_PLUGIN_SIZE: u64 = 1024 * 1024; // 1MB
        let metadata = tokio::fs::metadata(&entry_path)
            .await
            .map_err(|e| PluginError::LuaError(format!("stat {}: {e}", entry_path.display())))?;

        if metadata.len() > MAX_PLUGIN_SIZE {
            return Err(PluginError::LuaError(format!(
                "plugin {} code too large: {} bytes (max: {} bytes)",
                self.manifest.plugin.name,
                metadata.len(),
                MAX_PLUGIN_SIZE
            )));
        }

        let code = tokio::fs::read_to_string(&entry_path)
            .await
            .map_err(|e| PluginError::LuaError(format!("read {}: {e}", entry_path.display())))?;

        lua.load(&code)
            .exec()
            .map_err(|e| PluginError::InitFailed(format!("{}: {e}", self.manifest.plugin.name)))?;

        // Call sushi.init() if defined
        let sushi: mlua::Table = lua
            .globals()
            .get("sushi")
            .map_err(|e| PluginError::LuaError(format!("no sushi global: {e}")))?;

        if let Ok(init_fn) = sushi.get::<mlua::Function>("init") {
            init_fn.call::<()>(()).map_err(|e| {
                PluginError::InitFailed(format!("{}.init(): {e}", self.manifest.plugin.name))
            })?;
        }

        let plugin_name = &self.manifest.plugin.name;

        // Read pending routes, register with PluginManager
        if let Ok(pending) = sushi.get::<mlua::Table>("__pending_routes") {
            let len = pending.raw_len();
            for i in 1..=len {
                if let Ok(entry) = pending.get::<mlua::Table>(i) {
                    let method: String = entry.get("method").unwrap_or_default();
                    let path: String = entry.get("path").unwrap_or_default();
                    let handler_key: String = entry.get("handler_key").unwrap_or_default();
                    ctx.plugins
                        .register_api_handler(&method, &path, plugin_name, &handler_key)
                        .await;
                    tracing::debug!(
                        "plugin {} registered route {} {} (handler: {})",
                        plugin_name,
                        method,
                        path,
                        handler_key
                    );
                }
            }
        }

        // Read pending commands, register with PluginManager
        if let Ok(pending) = sushi.get::<mlua::Table>("__pending_commands") {
            let len = pending.raw_len();
            for i in 1..=len {
                if let Ok(entry) = pending.get::<mlua::Table>(i) {
                    let name: String = entry.get("name").unwrap_or_default();
                    let handler_key: String = entry.get("handler_key").unwrap_or_default();
                    ctx.plugins
                        .register_cli_handler(&name, plugin_name, &handler_key)
                        .await;
                    tracing::debug!(
                        "plugin {} registered command {} (handler: {})",
                        plugin_name,
                        name,
                        handler_key
                    );
                }
            }
        }

        // Read pending pages, register with PluginManager
        if let Ok(pending) = sushi.get::<mlua::Table>("__pending_pages") {
            let len = pending.raw_len();
            for i in 1..=len {
                if let Ok(entry) = pending.get::<mlua::Table>(i) {
                    let path: String = entry.get("path").unwrap_or_default();
                    let title: String = entry.get("title").unwrap_or_default();
                    let handler_key: String = entry.get("handler_key").unwrap_or_default();
                    ctx.plugins
                        .register_admin_handler(&path, plugin_name, &handler_key)
                        .await;
                    tracing::debug!(
                        "plugin {} registered page {} ({}) (handler: {})",
                        plugin_name,
                        path,
                        title,
                        handler_key
                    );
                }
            }
        }

        // Store the Lua VM in the PluginManager so handlers can be called later
        // We clone the lua ref (it's behind Option) — but we actually need to take ownership
        // Since Plugin trait uses &self, we use a workaround: register the VM separately
        drop(sushi);

        tracing::info!(
            "plugin loaded: {} v{}",
            plugin_name,
            self.manifest.plugin.version
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::jwt::JwtService;
    use crate::config::ConfigStore;
    use crate::storage::sqlite::SqliteStorage;
    use crate::web::template_service::TemplateService;
    use std::ops::Deref;
    use tempfile::TempDir;

    struct TestContext {
        ctx: SushiContext,
        _templates_dir: TempDir,
    }

    impl Deref for TestContext {
        type Target = SushiContext;

        fn deref(&self) -> &Self::Target {
            &self.ctx
        }
    }

    async fn test_context() -> TestContext {
        let config = ConfigStore::new(crate::config::SushiConfig::default());
        let db = SqliteStorage::new_in_memory().await.unwrap();
        let jwt = JwtService::new("test-secret-key-at-least-32-chars-long!", 3600, 604800);

        let templates_dir = tempfile::tempdir().unwrap();
        let templates = TemplateService::new(templates_dir.path()).unwrap();

        TestContext {
            ctx: SushiContext::new(config, db, jwt, templates),
            _templates_dir: templates_dir,
        }
    }

    fn create_plugin_dir(parent: &Path, name: &str) -> PathBuf {
        let dir = parent.join(name);
        std::fs::create_dir_all(&dir).unwrap();

        let manifest_content = format!(
            r#"
[plugin]
name = "{name}"
version = "0.1.0"
entry = "init.lua"

[permissions]
routes = true
"#,
        );
        std::fs::write(dir.join("plugin.toml"), manifest_content).unwrap();

        let init_lua = r#"
sushi.log.info("hello from plugin")
sushi.api.route("GET", "/api/test", function()
    return "ok"
end)
"#;
        std::fs::write(dir.join("init.lua"), init_lua).unwrap();

        dir
    }

    #[test]
    fn kv_store_plugin_no_longer_embeds_html() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
        let plugin_path = repo_root.join("plugins/kv-store/init.lua");
        let plugin_source = std::fs::read_to_string(&plugin_path).unwrap();

        assert!(!plugin_source.contains("<!DOCTYPE html>"));
        assert!(!plugin_source.contains("<html"));
        assert!(!plugin_source.contains("sushi.admin.page"));
        assert!(plugin_source.contains("sushi.web.page"));
        assert!(plugin_source.contains("plugins/kv-store/kv.html"));

        let template_path = repo_root.join("web/templates/plugins/kv-store/kv.html");
        assert!(template_path.exists());
        let template_source = std::fs::read_to_string(&template_path).unwrap();
        assert!(template_source.contains("{% extends \"base.html\" %}"));
        assert!(!template_source.contains("http://"));
        assert!(!template_source.contains("https://"));

        let static_path = repo_root.join("web/static/plugins/kv-store/kv.js");
        assert!(static_path.exists());
        let static_source = std::fs::read_to_string(&static_path).unwrap();
        assert!(static_source.contains("kvPage"));
        assert!(!static_source.contains("http://"));
        assert!(!static_source.contains("https://"));
    }

    #[tokio::test]
    async fn test_scan_dir_finds_plugins() {
        let tmp = TempDir::new().unwrap();
        create_plugin_dir(tmp.path(), "my_plugin");

        let plugins = LuaPlugin::scan_dir(tmp.path()).await.unwrap();
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].name(), "my_plugin");
        assert_eq!(plugins[0].version(), "0.1.0");
    }

    #[tokio::test]
    async fn test_scan_dir_skips_dirs_without_manifest() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("no_manifest");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("init.lua"), "print('hello')").unwrap();

        let plugins = LuaPlugin::scan_dir(tmp.path()).await.unwrap();
        assert_eq!(plugins.len(), 0);
    }

    #[tokio::test]
    async fn test_lua_plugin_init_executes_entry_script() {
        let tmp = TempDir::new().unwrap();
        create_plugin_dir(tmp.path(), "test_plugin");

        let plugins = LuaPlugin::scan_dir(tmp.path()).await.unwrap();
        let ctx = test_context().await;

        // init() should succeed without error
        plugins[0].init(&ctx).await.unwrap();
    }

    #[tokio::test]
    async fn test_lua_plugin_init_calls_sushi_init() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("init_fn_plugin");
        std::fs::create_dir_all(&dir).unwrap();

        std::fs::write(
            dir.join("plugin.toml"),
            r#"
[plugin]
name = "init_fn_plugin"
version = "0.1.0"
entry = "init.lua"
"#,
        )
        .unwrap();

        std::fs::write(
            dir.join("init.lua"),
            r#"
sushi.init = function()
    sushi.log.info("init called!")
end
"#,
        )
        .unwrap();

        let plugins = LuaPlugin::scan_dir(tmp.path()).await.unwrap();
        let ctx = test_context().await;

        plugins[0].init(&ctx).await.unwrap();
    }

    #[tokio::test]
    async fn test_lua_plugin_init_bad_manifest() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("bad_plugin");
        std::fs::create_dir_all(&dir).unwrap();

        // Invalid TOML
        std::fs::write(dir.join("plugin.toml"), "this is not valid toml [[[[").unwrap();

        let result = LuaPlugin::scan_dir(tmp.path()).await;
        assert!(result.is_err());
    }
}
