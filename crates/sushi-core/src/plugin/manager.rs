use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::{DatabasePermission, Permissions, PluginManifest};

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

#[derive(Debug, Clone, serde::Serialize)]
pub struct PluginAdminPageInfo {
    pub plugin: String,
    pub path: String,
    pub title: String,
}

#[derive(Debug, Clone, Default, serde::Serialize, PartialEq, Eq)]
pub struct PageResolvedAssets {
    pub js: Vec<String>,
    pub css: Vec<String>,
}

#[derive(Debug, Clone)]
struct ApiHandlerBinding {
    plugin_name: String,
    handler_key: String,
    policy_key: Option<String>,
    is_public: bool,
}

#[derive(Debug, Clone)]
struct CliHandlerBinding {
    plugin_name: String,
    handler_key: String,
    policy_key: Option<String>,
}

#[derive(Debug, Clone)]
struct AdminHandlerBinding {
    plugin_name: String,
    handler_key: String,
    policy_key: Option<String>,
    title: String,
    assets: PageResolvedAssets,
}

/// Manages loaded Lua plugin VMs and dispatches handler calls.
#[derive(Clone, Default)]
pub struct PluginManager {
    vms: Arc<RwLock<HashMap<String, mlua::Lua>>>,
    api_handlers: Arc<RwLock<HashMap<(String, String), ApiHandlerBinding>>>,
    cli_handlers: Arc<RwLock<HashMap<String, CliHandlerBinding>>>,
    admin_handlers: Arc<RwLock<HashMap<String, AdminHandlerBinding>>>,
    plugin_info: Arc<RwLock<HashMap<String, PluginInfo>>>,
    plugin_static_roots: Arc<RwLock<HashMap<String, PathBuf>>>,
}

fn match_api_handler_binding(
    map: &HashMap<(String, String), ApiHandlerBinding>,
    method: &str,
    path: &str,
) -> Option<ApiHandlerBinding> {
    let method_upper = method.to_uppercase();

    if let Some(binding) = map.get(&(method_upper.clone(), path.to_string())) {
        return Some(binding.clone());
    }

    let mut best_match = None;
    let mut longest_prefix_len = 0;
    for ((registered_method, registered_path), binding) in map.iter() {
        if registered_method == &method_upper && registered_path.ends_with('*') {
            let prefix = &registered_path[..registered_path.len() - 1];
            if path.starts_with(prefix) && prefix.len() > longest_prefix_len {
                longest_prefix_len = prefix.len();
                best_match = Some(binding.clone());
            }
        }
    }

    best_match
}

impl PluginManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn register_plugin_manifest(&self, manifest: &PluginManifest) {
        self.register_plugin_manifest_with_permissions(manifest, &manifest.permissions)
            .await;
    }

    pub async fn register_plugin_manifest_with_permissions(
        &self,
        manifest: &PluginManifest,
        effective_permissions: &Permissions,
    ) {
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
                    routes: effective_permissions.routes,
                    commands: effective_permissions.commands,
                    admin: effective_permissions.admin,
                    database: db_permission_name(&effective_permissions.database).to_string(),
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
        self.register_api_handler_with_policy_and_public(
            method,
            path,
            plugin_name,
            handler_key,
            None,
            false,
        )
        .await;
    }

    /// Register an API route handler with an optional policy key.
    pub async fn register_api_handler_with_policy(
        &self,
        method: &str,
        path: &str,
        plugin_name: &str,
        handler_key: &str,
        policy_key: Option<&str>,
    ) {
        self.register_api_handler_with_policy_and_public(
            method, path, plugin_name, handler_key, policy_key, false,
        )
        .await;
    }

    /// Register an API route handler with optional policy key and public flag.
    pub async fn register_api_handler_with_policy_and_public(
        &self,
        method: &str,
        path: &str,
        plugin_name: &str,
        handler_key: &str,
        policy_key: Option<&str>,
        is_public: bool,
    ) {
        self.api_handlers.write().await.insert(
            (method.to_uppercase(), path.to_string()),
            ApiHandlerBinding {
                plugin_name: plugin_name.to_string(),
                handler_key: handler_key.to_string(),
                policy_key: policy_key.map(ToOwned::to_owned),
                is_public,
            },
        );
    }

    /// Register a CLI command handler.
    pub async fn register_cli_handler(
        &self,
        command_name: &str,
        plugin_name: &str,
        handler_key: &str,
    ) {
        self.register_cli_handler_with_policy(command_name, plugin_name, handler_key, None)
            .await;
    }

    /// Register a CLI command handler with an optional policy key.
    pub async fn register_cli_handler_with_policy(
        &self,
        command_name: &str,
        plugin_name: &str,
        handler_key: &str,
        policy_key: Option<&str>,
    ) {
        self.cli_handlers.write().await.insert(
            command_name.to_string(),
            CliHandlerBinding {
                plugin_name: plugin_name.to_string(),
                handler_key: handler_key.to_string(),
                policy_key: policy_key.map(ToOwned::to_owned),
            },
        );
    }

    /// Register an admin page handler.
    pub async fn register_admin_handler(
        &self,
        page_path: &str,
        plugin_name: &str,
        title: &str,
        handler_key: &str,
    ) {
        self.register_admin_handler_with_policy(page_path, plugin_name, title, handler_key, None)
            .await;
    }

    /// Register an admin page handler with an optional policy key.
    pub async fn register_admin_handler_with_policy(
        &self,
        page_path: &str,
        plugin_name: &str,
        title: &str,
        handler_key: &str,
        policy_key: Option<&str>,
    ) {
        self.register_admin_handler_with_assets_and_policy(
            page_path,
            plugin_name,
            title,
            handler_key,
            PageResolvedAssets::default(),
            policy_key,
        )
        .await;
    }

    /// Register an admin page handler and resolved page assets.
    pub async fn register_admin_handler_with_assets(
        &self,
        page_path: &str,
        plugin_name: &str,
        title: &str,
        handler_key: &str,
        assets: PageResolvedAssets,
    ) {
        self.register_admin_handler_with_assets_and_policy(
            page_path,
            plugin_name,
            title,
            handler_key,
            assets,
            None,
        )
        .await;
    }

    /// Register an admin page handler and resolved page assets with an optional policy key.
    pub async fn register_admin_handler_with_assets_and_policy(
        &self,
        page_path: &str,
        plugin_name: &str,
        title: &str,
        handler_key: &str,
        assets: PageResolvedAssets,
        policy_key: Option<&str>,
    ) {
        self.admin_handlers.write().await.insert(
            page_path.to_string(),
            AdminHandlerBinding {
                plugin_name: plugin_name.to_string(),
                handler_key: handler_key.to_string(),
                policy_key: policy_key.map(ToOwned::to_owned),
                title: title.to_string(),
                assets,
            },
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
        self.dispatch_api_handler(
            method,
            path,
            path,
            body.map(|value| value.into_bytes()),
        )
        .await
    }

    /// Dispatch an API route handler using a dispatch path and optional binary request body.
    /// Route matching remains based on `path`, while Lua receives `dispatch_path`.
    pub async fn dispatch_api_handler(
        &self,
        method: &str,
        path: &str,
        dispatch_path: &str,
        body: Option<Vec<u8>>,
    ) -> Option<Result<String, String>> {
        let map = self.api_handlers.read().await;
        let binding = match_api_handler_binding(&map, method, path)?;
        let plugin_name = binding.plugin_name;
        let handler_key = binding.handler_key;
        drop(map);

        Some(
            self.call_api_handler_with_dispatch(
                &plugin_name,
                &handler_key,
                path,
                dispatch_path,
                body,
            )
            .await,
        )
    }

    /// Return the optional policy key for an API route registration.
    pub async fn api_route_policy(&self, method: &str, path: &str) -> Option<String> {
        let map = self.api_handlers.read().await;
        match_api_handler_binding(&map, method, path).and_then(|binding| binding.policy_key)
    }

    /// Return true when the API route was registered as public.
    pub async fn is_api_route_public(&self, method: &str, path: &str) -> bool {
        let map = self.api_handlers.read().await;
        match_api_handler_binding(&map, method, path)
            .map(|binding| binding.is_public)
            .unwrap_or(false)
    }

    /// Call a CLI command handler with string args, returns the output.
    pub async fn call_cli_handler(
        &self,
        command_name: &str,
        args: &[String],
    ) -> Option<Result<String, String>> {
        let binding = {
            let map = self.cli_handlers.read().await;
            map.get(command_name).cloned()?
        };
        Some(
            self.call_handler_with_args(&binding.plugin_name, &binding.handler_key, args)
                .await,
        )
    }

    /// Return the optional policy key for a CLI command registration.
    pub async fn cli_command_policy(&self, command_name: &str) -> Option<String> {
        self.cli_handlers
            .read()
            .await
            .get(command_name)
            .and_then(|binding| binding.policy_key.clone())
    }

    /// Call an admin page handler, returns HTML String.
    pub async fn call_admin_handler(&self, page_path: &str) -> Option<Result<String, String>> {
        let binding = {
            let map = self.admin_handlers.read().await;
            map.get(page_path).cloned()?
        };
        Some(
            self.call_handler_no_args(&binding.plugin_name, &binding.handler_key)
                .await,
        )
    }

    /// Return the optional policy key for an admin page registration.
    pub async fn admin_page_policy(&self, page_path: &str) -> Option<String> {
        self.admin_handlers
            .read()
            .await
            .get(page_path)
            .and_then(|binding| binding.policy_key.clone())
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

    /// List all registered admin pages for a specific plugin.
    pub async fn list_admin_pages_for_plugin(&self, plugin_name: &str) -> Vec<PluginAdminPageInfo> {
        let mut pages = self
            .admin_handlers
            .read()
            .await
            .iter()
            .filter_map(|(path, binding)| {
                if binding.plugin_name == plugin_name {
                    Some(PluginAdminPageInfo {
                        plugin: binding.plugin_name.clone(),
                        path: path.clone(),
                        title: binding.title.clone(),
                    })
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        pages.sort_by(|a, b| a.path.cmp(&b.path));
        pages
    }

    /// Get resolved JS/CSS assets for a registered admin page.
    pub async fn admin_page_assets(&self, page_path: &str) -> Option<PageResolvedAssets> {
        self.admin_handlers
            .read()
            .await
            .get(page_path)
            .map(|binding| binding.assets.clone())
    }

    /// Register plugin-scoped static assets root (`plugins/<name>/web/static`).
    pub async fn register_plugin_static_root(&self, plugin_name: &str, static_root: PathBuf) {
        self.plugin_static_roots
            .write()
            .await
            .insert(plugin_name.to_string(), static_root);
    }

    /// List all registered plugin static roots sorted by plugin name.
    pub async fn list_plugin_static_roots(&self) -> Vec<(String, PathBuf)> {
        let mut roots = self
            .plugin_static_roots
            .read()
            .await
            .iter()
            .map(|(name, root)| (name.clone(), root.clone()))
            .collect::<Vec<_>>();
        roots.sort_by(|a, b| a.0.cmp(&b.0));
        roots
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

    async fn call_api_handler_with_dispatch(
        &self,
        plugin_name: &str,
        handler_key: &str,
        path: &str,
        dispatch_path: &str,
        body: Option<Vec<u8>>,
    ) -> Result<String, String> {
        let vms = self.vms.read().await;
        let lua = vms
            .get(plugin_name)
            .ok_or_else(|| format!("plugin '{plugin_name}' not loaded"))?;

        let func = self.get_handler_fn(lua, handler_key)?;

        // Preserve existing `args[1]`/`args[2]` contract for compatibility.
        // Query-aware handlers can read `args.dispatch_path`.
        let args = lua
            .create_table()
            .map_err(|e| format!("create args table: {e}"))?;
        args.set(
            1,
            lua.create_string(path.as_bytes())
                .map_err(|e| format!("create path string: {e}"))?,
        )
        .map_err(|e| format!("set path arg: {e}"))?;
        if let Some(bytes) = body {
            args.set(
                2,
                lua.create_string(&bytes)
                    .map_err(|e| format!("create body string: {e}"))?,
            )
            .map_err(|e| format!("set body arg: {e}"))?;
        }
        args.set(
            "dispatch_path",
            lua.create_string(dispatch_path.as_bytes())
                .map_err(|e| format!("create dispatch path string: {e}"))?,
        )
        .map_err(|e| format!("set dispatch_path field: {e}"))?;

        func.call_async::<String>(args).await.map_err(|e| {
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
    use crate::plugin::{Permissions, PluginMeta, PluginPoliciesConfig};

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
            policies: PluginPoliciesConfig::default(),
            admin: None,
            file_browser: None,
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
            policies: PluginPoliciesConfig::default(),
            admin: None,
            file_browser: None,
        };
        manager.register_plugin_manifest(&manifest).await;

        let lua = mlua::Lua::new();
        manager.register_vm("example", lua).await;

        let plugins = manager.list_plugins().await;
        assert_eq!(plugins.len(), 1);
        assert!(plugins[0].loaded);
    }

    #[tokio::test]
    async fn list_admin_pages_for_plugin_returns_titles() {
        let manager = PluginManager::new();
        manager
            .register_admin_handler(
                "/admin/plugins/kv-store",
                "kv-store",
                "KV Store Workspace",
                "handler::workspace",
            )
            .await;
        manager
            .register_admin_handler("/admin/kv", "kv-store", "KV Store", "handler::kv")
            .await;
        manager
            .register_admin_handler("/admin/example", "example", "Example", "handler::example")
            .await;

        let pages = manager.list_admin_pages_for_plugin("kv-store").await;
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].plugin, "kv-store");
        assert_eq!(pages[0].title, "KV Store");
        assert_eq!(pages[1].title, "KV Store Workspace");
    }

    #[tokio::test]
    async fn admin_page_assets_are_stored_and_returned() {
        let manager = PluginManager::new();
        manager
            .register_admin_handler_with_assets(
                "/admin/plugins/kv-store",
                "kv-store",
                "KV Store Workspace",
                "handler::workspace",
                PageResolvedAssets {
                    js: vec!["/static/plugins/official/kv-store/kv.js".to_string()],
                    css: vec!["/static/plugins/official/kv-store/kv.css".to_string()],
                },
            )
            .await;

        let assets = manager
            .admin_page_assets("/admin/plugins/kv-store")
            .await
            .expect("expected assets for registered page");

        assert_eq!(
            assets.js,
            vec!["/static/plugins/official/kv-store/kv.js".to_string()]
        );
        assert_eq!(
            assets.css,
            vec!["/static/plugins/official/kv-store/kv.css".to_string()]
        );
    }

    #[tokio::test]
    async fn list_plugin_static_roots_returns_sorted_entries() {
        let manager = PluginManager::new();
        manager
            .register_plugin_static_root("zeta", PathBuf::from("/tmp/zeta"))
            .await;
        manager
            .register_plugin_static_root("alpha", PathBuf::from("/tmp/alpha"))
            .await;

        let roots = manager.list_plugin_static_roots().await;
        assert_eq!(roots.len(), 2);
        assert_eq!(roots[0].0, "alpha");
        assert_eq!(roots[1].0, "zeta");
    }

    #[tokio::test]
    async fn register_plugin_manifest_with_effective_permissions_uses_effective_values() {
        let manager = PluginManager::new();
        let manifest = PluginManifest {
            plugin: PluginMeta {
                name: "kv-store".to_string(),
                version: "1.0.0".to_string(),
                description: "KV plugin".to_string(),
                entry: "init.lua".to_string(),
            },
            permissions: Permissions::default(),
            policies: PluginPoliciesConfig::default(),
            admin: None,
            file_browser: None,
        };

        let effective = Permissions {
            routes: true,
            commands: true,
            admin: true,
            database: DatabasePermission::Admin,
        };

        manager
            .register_plugin_manifest_with_permissions(&manifest, &effective)
            .await;
        let plugins = manager.list_plugins().await;

        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].permissions.database, "admin");
        assert!(plugins[0].permissions.routes);
        assert!(plugins[0].permissions.commands);
        assert!(plugins[0].permissions.admin);
    }

    #[tokio::test]
    async fn registration_policy_metadata_is_stored() {
        let manager = PluginManager::new();
        manager
            .register_api_handler_with_policy_and_public(
                "GET",
                "/api/notes",
                "notes",
                "handler::api",
                Some("api.notes.read"),
                true,
            )
            .await;
        manager
            .register_api_handler("POST", "/api/notes", "notes", "handler::api-post")
            .await;
        manager
            .register_cli_handler_with_policy(
                "notes-run",
                "notes",
                "handler::cli",
                Some("cli.notes.run"),
            )
            .await;
        manager
            .register_cli_handler("notes-list", "notes", "handler::cli-list")
            .await;
        manager
            .register_admin_handler_with_assets_and_policy(
                "/admin/notes",
                "notes",
                "Notes",
                "handler::admin",
                PageResolvedAssets::default(),
                Some("admin.notes.read"),
            )
            .await;
        manager
            .register_admin_handler("/admin/notes-open", "notes", "Notes Open", "handler::open")
            .await;

        assert_eq!(
            manager
                .api_route_policy("GET", "/api/notes")
                .await
                .as_deref(),
            Some("api.notes.read")
        );
        assert!(manager.is_api_route_public("GET", "/api/notes").await);
        assert_eq!(
            manager
                .api_route_policy("POST", "/api/notes")
                .await
                .as_deref(),
            None
        );
        assert!(!manager.is_api_route_public("POST", "/api/notes").await);
        assert_eq!(
            manager.cli_command_policy("notes-run").await.as_deref(),
            Some("cli.notes.run")
        );
        assert_eq!(
            manager.cli_command_policy("notes-list").await.as_deref(),
            None
        );
        assert_eq!(
            manager.admin_page_policy("/admin/notes").await.as_deref(),
            Some("admin.notes.read")
        );
        assert_eq!(
            manager
                .admin_page_policy("/admin/notes-open")
                .await
                .as_deref(),
            None
        );
    }

    #[tokio::test]
    async fn api_route_policy_matches_wildcard_for_concrete_path() {
        let manager = PluginManager::new();
        manager
            .register_api_handler_with_policy_and_public(
                "GET",
                "/api/*",
                "notes",
                "handler::api-root",
                Some("api.root.read"),
                false,
            )
            .await;
        manager
            .register_api_handler_with_policy_and_public(
                "GET",
                "/api/notes/*",
                "notes",
                "handler::api-notes",
                Some("api.notes.read"),
                true,
            )
            .await;

        assert_eq!(
            manager
                .api_route_policy("GET", "/api/notes/123")
                .await
                .as_deref(),
            Some("api.notes.read")
        );
        assert!(manager.is_api_route_public("GET", "/api/notes/123").await);
    }

    #[tokio::test]
    async fn dispatch_api_handler_forwards_dispatch_path_and_binary_body() {
        let manager = PluginManager::new();
        let lua = mlua::Lua::new();

        let sushi = lua.create_table().unwrap();
        let handlers = lua.create_table().unwrap();
        sushi.set("__handlers", handlers.clone()).unwrap();
        lua.globals().set("sushi", sushi).unwrap();

        let handler = lua
            .create_async_function(|_, args: mlua::Table| async move {
                let path: String = args.get(1)?;
                let dispatch_path: String = args.get("dispatch_path")?;
                let body: mlua::String = args.get(2)?;
                Ok(format!(
                    "{}|{}|{}:{}",
                    path,
                    dispatch_path,
                    body.as_bytes()[0],
                    body.as_bytes()[1]
                ))
            })
            .unwrap();
        handlers.set("h_dispatch", handler).unwrap();

        manager.register_vm("plugin", lua).await;
        manager
            .register_api_handler("POST", "/api/upload", "plugin", "h_dispatch")
            .await;

        let result = manager
            .dispatch_api_handler(
                "POST",
                "/api/upload",
                "/api/upload?mode=raw",
                Some(vec![0, 255]),
            )
            .await
            .expect("handler must exist")
            .expect("handler must run");
        assert_eq!(result, "/api/upload|/api/upload?mode=raw|0:255");
    }

    #[tokio::test]
    async fn call_api_handler_matches_wildcards() {
        let manager = PluginManager::new();
        let lua = mlua::Lua::new();
        let sushi = lua.create_table().expect("create sushi table");
        let handlers = lua.create_table().expect("create handlers table");
        sushi
            .set("__handlers", handlers.clone())
            .expect("set handlers table");
        lua.globals()
            .set("sushi", sushi)
            .expect("set sushi global");
        lua.load(
            r#"
            sushi.__handlers["h_wildcard"] = function(args)
                return args[1]
            end
            "#,
        )
        .exec()
        .expect("register wildcard handler");

        manager.register_vm("notes", lua).await;
        manager
            .register_api_handler("GET", "/api/notes/*", "notes", "h_wildcard")
            .await;

        let response = manager
            .call_api_handler("GET", "/api/notes/123", None)
            .await
            .expect("expected wildcard handler")
            .expect("expected successful handler call");
        assert_eq!(response, "/api/notes/123");
    }
}
