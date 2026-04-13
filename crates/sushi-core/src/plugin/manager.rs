use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::{DatabasePermission, PluginManifest};

#[derive(Debug, Clone, serde::Serialize)]
pub struct PluginPermissionsView {
    pub routes: bool,
    pub commands: bool,
    pub admin: bool,
    pub database: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub loaded: bool,
    pub permissions: PluginPermissionsView,
}

/// Manages loaded Lua plugin VMs and dispatches handler calls.
#[derive(Clone, Default)]
pub struct PluginManager {
    vms: Arc<RwLock<HashMap<String, mlua::Lua>>>,
    api_handlers: Arc<RwLock<HashMap<(String, String), (String, String)>>>,
    cli_handlers: Arc<RwLock<HashMap<String, (String, String)>>>,
    admin_handlers: Arc<RwLock<HashMap<String, (String, String)>>>,
    plugin_info: Arc<RwLock<HashMap<String, PluginInfo>>>,
}

impl PluginManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn register_plugin_manifest(&self, manifest: &PluginManifest) {
        let mut plugin_info = self.plugin_info.write().await;
        let loaded = plugin_info
            .get(&manifest.plugin.name)
            .map(|item| item.loaded)
            .unwrap_or(false);

        plugin_info.insert(
            manifest.plugin.name.clone(),
            PluginInfo {
                name: manifest.plugin.name.clone(),
                version: manifest.plugin.version.clone(),
                description: manifest.plugin.description.clone(),
                loaded,
                permissions: PluginPermissionsView {
                    routes: manifest.permissions.routes,
                    commands: manifest.permissions.commands,
                    admin: manifest.permissions.admin,
                    database: db_permission_name(&manifest.permissions.database).to_string(),
                },
            },
        );
    }

    pub async fn mark_plugin_loaded(&self, plugin_name: &str, loaded: bool) {
        let mut plugin_info = self.plugin_info.write().await;
        if let Some(item) = plugin_info.get_mut(plugin_name) {
            item.loaded = loaded;
            return;
        }

        plugin_info.insert(
            plugin_name.to_string(),
            PluginInfo {
                name: plugin_name.to_string(),
                version: String::new(),
                description: String::new(),
                loaded,
                permissions: PluginPermissionsView {
                    routes: false,
                    commands: false,
                    admin: false,
                    database: db_permission_name(&DatabasePermission::None).to_string(),
                },
            },
        );
    }

    pub async fn list_plugins(&self) -> Vec<PluginInfo> {
        let mut plugins = self
            .plugin_info
            .read()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>();
        plugins.sort_by(|a, b| a.name.cmp(&b.name));
        plugins
    }

    /// Store a loaded Lua VM for a plugin.
    pub async fn register_vm(&self, plugin_name: &str, lua: mlua::Lua) {
        self.vms.write().await.insert(plugin_name.to_string(), lua);
        self.mark_plugin_loaded(plugin_name, true).await;
    }

    /// Register an API route handler.
    pub async fn register_api_handler(
        &self,
        method: &str,
        path: &str,
        plugin_name: &str,
        handler_key: &str,
    ) {
        self.api_handlers.write().await.insert(
            (method.to_uppercase(), path.to_string()),
            (plugin_name.to_string(), handler_key.to_string()),
        );
    }

    /// Register a CLI command handler.
    pub async fn register_cli_handler(
        &self,
        command_name: &str,
        plugin_name: &str,
        handler_key: &str,
    ) {
        self.cli_handlers.write().await.insert(
            command_name.to_string(),
            (plugin_name.to_string(), handler_key.to_string()),
        );
    }

    /// Register an admin page handler.
    pub async fn register_admin_handler(
        &self,
        page_path: &str,
        plugin_name: &str,
        handler_key: &str,
    ) {
        self.admin_handlers.write().await.insert(
            page_path.to_string(),
            (plugin_name.to_string(), handler_key.to_string()),
        );
    }

    /// Call an API route handler, returns the response body as String.
    /// `body` is the raw request body (if any), passed to the Lua handler.
    /// Supports exact match and prefix wildcard (path ending with `*`).
    pub async fn call_api_handler(
        &self,
        method: &str,
        path: &str,
        body: Option<String>,
    ) -> Option<Result<String, String>> {
        let method_upper = method.to_uppercase();
        let map = self.api_handlers.read().await;

        // Try exact match first
        let (plugin_name, handler_key) =
            if let Some(v) = map.get(&(method_upper.clone(), path.to_string())) {
                v.clone()
            } else {
                // Try prefix wildcard match: find entries where path ends with *
                let mut best_match = None;
                let mut longest_prefix_len = 0;
                for ((m, prefix), val) in map.iter() {
                    if m == &method_upper && prefix.ends_with('*') {
                        let prefix_str = &prefix[..prefix.len() - 1];
                        if path.starts_with(prefix_str) {
                            if prefix_str.len() > longest_prefix_len {
                                longest_prefix_len = prefix_str.len();
                                best_match = Some(val.clone());
                            }
                        }
                    }
                }
                best_match?
            };
        drop(map);

        // Pass path + body as args so Lua handler can extract path params
        let args = match body {
            Some(b) => vec![path.to_string(), b],
            None => vec![path.to_string()],
        };
        Some(
            self.call_handler_with_args(&plugin_name, &handler_key, &args)
                .await,
        )
    }

    /// Call a CLI command handler with string args, returns the output.
    pub async fn call_cli_handler(
        &self,
        command_name: &str,
        args: &[String],
    ) -> Option<Result<String, String>> {
        let (plugin_name, handler_key) = {
            let map = self.cli_handlers.read().await;
            map.get(command_name).cloned()?
        };
        Some(
            self.call_handler_with_args(&plugin_name, &handler_key, args)
                .await,
        )
    }

    /// Call an admin page handler, returns HTML String.
    pub async fn call_admin_handler(&self, page_path: &str) -> Option<Result<String, String>> {
        let (plugin_name, handler_key) = {
            let map = self.admin_handlers.read().await;
            map.get(page_path).cloned()?
        };
        Some(self.call_handler_no_args(&plugin_name, &handler_key).await)
    }

    /// List all registered API routes.
    pub async fn list_api_routes(&self) -> Vec<(String, String)> {
        self.api_handlers
            .read()
            .await
            .keys()
            .map(|(m, p)| (m.clone(), p.clone()))
            .collect()
    }

    /// List all registered CLI commands.
    pub async fn list_cli_commands(&self) -> Vec<String> {
        self.cli_handlers.read().await.keys().cloned().collect()
    }

    /// List all registered admin pages.
    pub async fn list_admin_pages(&self) -> Vec<String> {
        self.admin_handlers.read().await.keys().cloned().collect()
    }

    // -- private helpers --

    async fn call_handler_no_args(
        &self,
        plugin_name: &str,
        handler_key: &str,
    ) -> Result<String, String> {
        let vms = self.vms.read().await;
        let lua = vms
            .get(plugin_name)
            .ok_or_else(|| format!("plugin '{plugin_name}' not loaded"))?;

        let func = self.get_handler_fn(lua, handler_key)?;
        func.call_async::<String>(()).await.map_err(|e| {
            tracing::error!(
                "lua plugin handler failed: plugin={plugin_name} handler={handler_key} error={e}"
            );
            format!("handler error: {e}")
        })
    }

    async fn call_handler_with_args(
        &self,
        plugin_name: &str,
        handler_key: &str,
        args: &[String],
    ) -> Result<String, String> {
        let vms = self.vms.read().await;
        let lua = vms
            .get(plugin_name)
            .ok_or_else(|| format!("plugin '{plugin_name}' not loaded"))?;

        let func = self.get_handler_fn(lua, handler_key)?;

        // Build a Lua table from args
        let args_table = lua
            .create_table()
            .map_err(|e| format!("create args table: {e}"))?;
        for (i, arg) in args.iter().enumerate() {
            args_table
                .set(i + 1, arg.clone())
                .map_err(|e| format!("set arg: {e}"))?;
        }

        func.call_async::<String>(args_table).await.map_err(|e| {
            tracing::error!(
                "lua plugin handler failed: plugin={plugin_name} handler={handler_key} error={e}"
            );
            format!("handler error: {e}")
        })
    }

    fn get_handler_fn(&self, lua: &mlua::Lua, handler_key: &str) -> Result<mlua::Function, String> {
        let sushi: mlua::Table = lua
            .globals()
            .get("sushi")
            .map_err(|e| format!("no sushi global: {e}"))?;

        let handlers: mlua::Table = sushi
            .get("__handlers")
            .map_err(|e| format!("no __handlers table: {e}"))?;

        handlers
            .get(handler_key)
            .map_err(|e| format!("handler '{handler_key}' not found: {e}"))
    }
}

fn db_permission_name(permission: &DatabasePermission) -> &'static str {
    match permission {
        DatabasePermission::None => "none",
        DatabasePermission::ReadOnly => "read",
        DatabasePermission::Write => "write",
        DatabasePermission::Admin => "admin",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::{Permissions, PluginMeta};

    #[tokio::test]
    async fn register_manifest_is_visible_in_plugin_list() {
        let manager = PluginManager::new();
        let manifest = PluginManifest {
            plugin: PluginMeta {
                name: "kv-store".to_string(),
                version: "1.0.0".to_string(),
                description: "KV plugin".to_string(),
                entry: "init.lua".to_string(),
            },
            permissions: Permissions {
                routes: true,
                commands: true,
                admin: true,
                database: DatabasePermission::Write,
            },
        };

        manager.register_plugin_manifest(&manifest).await;
        let plugins = manager.list_plugins().await;
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].name, "kv-store");
        assert_eq!(plugins[0].version, "1.0.0");
        assert!(!plugins[0].loaded);
        assert_eq!(plugins[0].permissions.database, "write");
    }

    #[tokio::test]
    async fn register_vm_marks_plugin_as_loaded() {
        let manager = PluginManager::new();
        let manifest = PluginManifest {
            plugin: PluginMeta {
                name: "example".to_string(),
                version: "0.1.0".to_string(),
                description: "Example plugin".to_string(),
                entry: "init.lua".to_string(),
            },
            permissions: Permissions::default(),
        };
        manager.register_plugin_manifest(&manifest).await;

        let lua = mlua::Lua::new();
        manager.register_vm("example", lua).await;

        let plugins = manager.list_plugins().await;
        assert_eq!(plugins.len(), 1);
        assert!(plugins[0].loaded);
    }
}
