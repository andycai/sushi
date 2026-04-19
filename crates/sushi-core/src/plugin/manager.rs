use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{Mutex, OwnedMutexGuard, RwLock};

use super::state_repository::PluginStateRepository;
use super::{DatabasePermission, Permissions, PluginKind, PluginManifest};

#[derive(Debug, Clone, serde::Serialize)]
pub struct PluginPermissionsView {
    pub routes: bool,
    pub commands: bool,
    pub admin: bool,
    pub database: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PluginInfo {
    pub plugin_id: String,
    pub source_kind: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub enabled: bool,
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
    state_repo: Option<Arc<PluginStateRepository>>,
    plugin_runtime_locks: Arc<RwLock<HashMap<String, Arc<Mutex<()>>>>>,
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

    pub fn new_with_storage(storage: Arc<dyn crate::storage::Storage>) -> Self {
        Self {
            state_repo: Some(Arc::new(PluginStateRepository::new(storage))),
            ..Self::default()
        }
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
        self.register_plugin_manifest_with_permissions_and_identity(
            manifest,
            effective_permissions,
            &manifest.plugin.name,
            PluginKind::ThirdParty,
        )
        .await;
    }

    pub async fn register_plugin_manifest_with_permissions_and_identity(
        &self,
        manifest: &PluginManifest,
        effective_permissions: &Permissions,
        plugin_id: &str,
        kind: PluginKind,
    ) {
        let plugin_name = manifest.plugin.name.clone();
        let manifest_version = manifest.plugin.version.clone();
        let source_kind = kind.tier_name().to_string();

        let existing = self.plugin_info.read().await.get(&plugin_name).cloned();
        let mut resolved_plugin_id = plugin_id.to_string();
        let mut resolved_source_kind = source_kind.clone();
        let mut enabled = existing.as_ref().map(|item| item.enabled).unwrap_or(true);
        let mut loaded = existing.as_ref().map(|item| item.loaded).unwrap_or(false);
        let mut resolved_version = manifest_version.clone();

        // Best-effort identity upsert: runtime state must not block plugin bootstrap.
        if let Some(repo) = &self.state_repo {
            if let Err(err) = repo
                .upsert_discovered_plugin(plugin_id, &plugin_name, &source_kind, &manifest_version)
                .await
            {
                tracing::warn!(
                    "failed to upsert plugin runtime state during manifest registration: plugin={} error={}",
                    plugin_name,
                    err
                );
            }

            match repo.get_by_name(&plugin_name).await {
                Ok(Some(state)) => {
                    resolved_plugin_id = state.plugin_id;
                    resolved_source_kind = state.source_kind;
                    enabled = state.enabled;
                    loaded = state.loaded;
                    if !state.version.is_empty() {
                        resolved_version = state.version;
                    }
                }
                Ok(None) => {}
                Err(err) => {
                    tracing::warn!(
                        "failed to read plugin runtime state during manifest registration: plugin={} error={}",
                        plugin_name,
                        err
                    );
                }
            }
        }

        self.plugin_info.write().await.insert(
            plugin_name.clone(),
            PluginInfo {
                plugin_id: resolved_plugin_id,
                source_kind: resolved_source_kind,
                name: plugin_name,
                version: resolved_version,
                description: manifest.plugin.description.clone(),
                enabled,
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
                plugin_id: plugin_name.to_string(),
                source_kind: "third_party".to_string(),
                name: plugin_name.to_string(),
                version: String::new(),
                description: String::new(),
                enabled: true,
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

        if let Some(repo) = &self.state_repo {
            for plugin in &mut plugins {
                match repo.get_by_name(&plugin.name).await {
                    Ok(Some(state)) => {
                        plugin.plugin_id = state.plugin_id;
                        plugin.source_kind = state.source_kind;
                        plugin.enabled = state.enabled;
                        plugin.loaded = state.loaded;
                        if !state.version.is_empty() {
                            plugin.version = state.version;
                        }
                    }
                    Ok(None) => {}
                    Err(err) => {
                        tracing::warn!(
                            "failed to read plugin runtime state while listing plugins: plugin={} error={}",
                            plugin.name,
                            err
                        );
                    }
                }
            }

            // Keep in-memory cache aligned with storage-backed source of truth.
            let mut plugin_info = self.plugin_info.write().await;
            for plugin in &plugins {
                if let Some(item) = plugin_info.get_mut(&plugin.name) {
                    item.plugin_id = plugin.plugin_id.clone();
                    item.source_kind = plugin.source_kind.clone();
                    item.version = plugin.version.clone();
                    item.enabled = plugin.enabled;
                    item.loaded = plugin.loaded;
                }
            }
        }

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
            method,
            path,
            plugin_name,
            handler_key,
            policy_key,
            false,
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
        self.dispatch_api_handler(method, path, path, body.map(|value| value.into_bytes()))
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

        if let Err(err) = self.guard_plugin_enabled(&plugin_name).await {
            return Some(Err(err));
        }

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
        if let Err(err) = self.guard_plugin_enabled(&binding.plugin_name).await {
            return Some(Err(err));
        }
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
        if let Err(err) = self.guard_plugin_enabled(&binding.plugin_name).await {
            return Some(Err(err));
        }
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

    pub async fn set_plugin_enabled(
        &self,
        plugin_name: &str,
        enabled: bool,
        actor: Option<&str>,
        reason: Option<&str>,
    ) -> Result<PluginInfo, String> {
        let _runtime_guard = self.acquire_plugin_runtime_lock(plugin_name).await;
        let known_plugin = self
            .plugin_info
            .read()
            .await
            .get(plugin_name)
            .cloned()
            .ok_or_else(|| format!("plugin not found: {plugin_name}"))?;

        if let Some(repo) = &self.state_repo {
            repo.upsert_discovered_plugin(
                &known_plugin.plugin_id,
                &known_plugin.name,
                &known_plugin.source_kind,
                &known_plugin.version,
            )
            .await?;
            let state = repo.set_enabled(plugin_name, enabled, actor, reason).await?;
            let mut info = self.plugin_info.write().await;
            let item = info
                .entry(plugin_name.to_string())
                .or_insert_with(|| known_plugin.clone());
            item.plugin_id = state.plugin_id;
            item.source_kind = state.source_kind;
            if !state.version.is_empty() {
                item.version = state.version;
            }
            item.enabled = state.enabled;
            item.loaded = state.loaded;
            return Ok(item.clone());
        }

        let mut info = self.plugin_info.write().await;
        if let Some(item) = info.get_mut(plugin_name) {
            item.enabled = enabled;
            return Ok(item.clone());
        }

        let mut fallback = known_plugin;
        fallback.enabled = enabled;
        info.insert(plugin_name.to_string(), fallback.clone());
        Ok(fallback)
    }

    pub async fn acquire_plugin_runtime_lock(
        &self,
        plugin_name: &str,
    ) -> OwnedMutexGuard<()> {
        let lock = {
            let mut locks = self.plugin_runtime_locks.write().await;
            locks
                .entry(plugin_name.to_string())
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone()
        };
        lock.lock_owned().await
    }

    pub async fn plugin_runtime_enabled(&self, plugin_name: &str) -> Result<bool, String> {
        if let Some(repo) = &self.state_repo {
            if let Some(state) = repo.get_by_name(plugin_name).await? {
                let mut info = self.plugin_info.write().await;
                if let Some(plugin) = info.get_mut(plugin_name) {
                    plugin.plugin_id = state.plugin_id.clone();
                    plugin.source_kind = state.source_kind.clone();
                    if !state.version.is_empty() {
                        plugin.version = state.version.clone();
                    }
                    plugin.enabled = state.enabled;
                    plugin.loaded = state.loaded;
                } else {
                    info.insert(
                        plugin_name.to_string(),
                        PluginInfo {
                            plugin_id: state.plugin_id.clone(),
                            source_kind: state.source_kind.clone(),
                            name: state.name.clone(),
                            version: state.version.clone(),
                            description: String::new(),
                            enabled: state.enabled,
                            loaded: state.loaded,
                            permissions: PluginPermissionsView {
                                routes: false,
                                commands: false,
                                admin: false,
                                database: db_permission_name(&DatabasePermission::None)
                                    .to_string(),
                            },
                        },
                    );
                }
                return Ok(state.enabled);
            }
        }

        self.plugin_info
            .read()
            .await
            .get(plugin_name)
            .map(|plugin| plugin.enabled)
            .ok_or_else(|| format!("plugin not found: {plugin_name}"))
    }

    // -- private helpers --

    async fn guard_plugin_enabled(&self, plugin_name: &str) -> Result<(), String> {
        if !self.plugin_runtime_enabled(plugin_name).await? {
            return Err(format!("plugin_disabled: plugin '{plugin_name}' is disabled"));
        }
        Ok(())
    }

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
    use crate::plugin::state_repository::PluginStateRepository;
    use crate::plugin::{Permissions, PluginMeta, PluginPoliciesConfig};
    use crate::storage::sqlite::SqliteStorage;
    use crate::storage::Storage;
    use std::sync::Arc;

    async fn storage_with_governance_schema() -> Arc<SqliteStorage> {
        let sqlite = Arc::new(SqliteStorage::new_in_memory().await.expect("create sqlite"));
        sqlite
            .run_migrations(include_str!("../../../../migrations/001_init.sql"))
            .await
            .expect("run migration 001");

        sqlite
            .run_migrations(
                r#"
                CREATE TABLE IF NOT EXISTS roles (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    slug TEXT NOT NULL UNIQUE
                );

                CREATE TABLE IF NOT EXISTS policy_keys (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    key TEXT NOT NULL UNIQUE,
                    surface TEXT NOT NULL,
                    resource TEXT NOT NULL,
                    action TEXT NOT NULL,
                    name TEXT NOT NULL,
                    description TEXT NOT NULL DEFAULT '',
                    is_system INTEGER NOT NULL DEFAULT 0
                );

                CREATE TABLE IF NOT EXISTS role_policy_keys (
                    role_id INTEGER NOT NULL,
                    policy_key_id INTEGER NOT NULL,
                    UNIQUE(role_id, policy_key_id)
                );

                CREATE TABLE IF NOT EXISTS policy_bindings (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    surface TEXT NOT NULL,
                    target_type TEXT NOT NULL,
                    target_ref TEXT NOT NULL,
                    method TEXT,
                    path_pattern TEXT,
                    command_name TEXT,
                    policy_key_id INTEGER NOT NULL,
                    owner_type TEXT NOT NULL,
                    owner_id TEXT NOT NULL,
                    is_system INTEGER NOT NULL DEFAULT 0
                );
                "#,
            )
            .await
            .expect("create migration prerequisites");

        sqlite
            .run_migrations(include_str!("../../../../migrations/008_plugin_governance_v1.sql"))
            .await
            .expect("run migration 008");

        sqlite
    }

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
        assert_eq!(plugins[0].plugin_id, "kv-store");
        assert_eq!(plugins[0].source_kind, "third_party");
        assert_eq!(plugins[0].version, "1.0.0");
        assert!(plugins[0].enabled);
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
        lua.globals().set("sushi", sushi).expect("set sushi global");
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

    #[tokio::test]
    async fn plugin_disabled_gate_blocks_api_admin_and_cli_dispatch() {
        let manager = PluginManager::new();
        let lua = mlua::Lua::new();

        let sushi = lua.create_table().expect("create sushi table");
        let handlers = lua.create_table().expect("create handlers table");
        sushi
            .set("__handlers", handlers.clone())
            .expect("set handlers table");
        lua.globals().set("sushi", sushi).expect("set sushi global");

        let passthrough = lua
            .create_async_function(|_, _: mlua::Value| async move { Ok("ok".to_string()) })
            .expect("create passthrough handler");
        handlers
            .set("h", passthrough)
            .expect("register passthrough handler");

        manager.register_vm("notes", lua).await;
        manager
            .register_api_handler("GET", "/api/notes", "notes", "h")
            .await;
        manager
            .register_admin_handler("/admin/notes", "notes", "Notes", "h")
            .await;
        manager
            .register_cli_handler("notes-run", "notes", "h")
            .await;

        manager
            .set_plugin_enabled("notes", false, Some("admin"), Some("test"))
            .await
            .expect("disable plugin");

        let api = manager
            .call_api_handler("GET", "/api/notes", None)
            .await
            .expect("api binding must exist")
            .expect_err("disabled plugin must fail");
        let admin = manager
            .call_admin_handler("/admin/notes")
            .await
            .expect("admin binding must exist")
            .expect_err("disabled plugin must fail");
        let cli = manager
            .call_cli_handler("notes-run", &[])
            .await
            .expect("cli binding must exist")
            .expect_err("disabled plugin must fail");

        assert!(api.contains("plugin_disabled"));
        assert!(admin.contains("plugin_disabled"));
        assert!(cli.contains("plugin_disabled"));
    }

    #[tokio::test]
    async fn storage_backed_set_plugin_enabled_updates_repo_and_list() {
        let sqlite = storage_with_governance_schema().await;
        let storage: Arc<dyn Storage> = sqlite.clone();
        let manager = PluginManager::new_with_storage(storage.clone());
        let repo = PluginStateRepository::new(storage);

        let manifest = PluginManifest {
            plugin: PluginMeta {
                name: "notes".to_string(),
                version: "0.1.0".to_string(),
                description: "Notes plugin".to_string(),
                entry: "init.lua".to_string(),
            },
            permissions: Permissions::default(),
            policies: PluginPoliciesConfig::default(),
            admin: None,
            file_browser: None,
        };

        manager.register_plugin_manifest(&manifest).await;
        let updated = manager
            .set_plugin_enabled("notes", false, Some("admin"), Some("maintenance"))
            .await
            .expect("disable plugin");

        assert!(!updated.enabled);

        let stored = repo
            .get_by_name("notes")
            .await
            .expect("query stored plugin state")
            .expect("stored plugin row");
        assert!(!stored.enabled);

        let listed = manager.list_plugins().await;
        assert_eq!(listed.len(), 1);
        assert!(!listed[0].enabled);
        assert_eq!(listed[0].plugin_id, stored.plugin_id);
    }

    #[tokio::test]
    async fn set_plugin_enabled_missing_plugin_does_not_write_repo() {
        let sqlite = storage_with_governance_schema().await;
        let storage: Arc<dyn Storage> = sqlite;
        let manager = PluginManager::new_with_storage(storage.clone());
        let repo = PluginStateRepository::new(storage);

        let err = manager
            .set_plugin_enabled("missing", false, Some("admin"), Some("noop"))
            .await
            .expect_err("missing plugin should fail");
        assert!(err.contains("plugin not found"));

        let stored = repo
            .get_by_name("missing")
            .await
            .expect("query missing plugin state");
        assert!(stored.is_none());
    }
}
