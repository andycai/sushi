use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{Mutex, OwnedMutexGuard, RwLock};

use super::state_repository::PluginStateRepository;
use super::{DatabasePermission, Permissions, PluginKind, PluginManifest};
use crate::runtime::{
    AdminPageSpec, CapabilityRegistry, CapabilitySnapshot, CliCommandSpec, HttpRequest,
    HttpResponse, HttpRouteSpec, LuaRuntimeInstance, PendingCapabilityCommit, PluginInstanceId,
    RegistrationConflict, RegistrationSource, StagedRegistrar, StaticRootSpec,
};

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

/// Manages loaded Lua plugin VMs and dispatches handler calls.
#[derive(Clone, Default)]
pub struct PluginManager {
    vms: Arc<RwLock<HashMap<String, Arc<LuaRuntimeInstance>>>>,
    plugin_info: Arc<RwLock<HashMap<String, PluginInfo>>>,
    required_plugins: Arc<RwLock<HashSet<String>>>,
    capabilities: CapabilityRegistry,
    state_repo: Option<Arc<PluginStateRepository>>,
    plugin_runtime_locks: Arc<RwLock<HashMap<String, Arc<Mutex<()>>>>>,
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

    pub fn new_with_sqlite_storage(storage: Arc<crate::storage::sqlite::SqliteStorage>) -> Self {
        Self {
            state_repo: Some(Arc::new(PluginStateRepository::new_sqlite(storage))),
            ..Self::default()
        }
    }

    pub async fn register_plugin_manifest(&self, manifest: &PluginManifest) {
        self.register_plugin_manifest_with_permissions(manifest, &manifest.permissions)
            .await;
    }

    pub async fn register_builtin_profile_plugin(
        &self,
        plugin_id: &str,
        plugin_name: &str,
        version: &str,
        description: &str,
        permissions: &Permissions,
        default_enabled: bool,
        required: bool,
    ) {
        self.set_plugin_required(plugin_name, required).await;
        let existing = self.plugin_info.read().await.get(plugin_name).cloned();
        let mut resolved_plugin_id = plugin_id.to_string();
        let mut enabled = existing
            .as_ref()
            .map(|plugin| plugin.enabled)
            .unwrap_or(default_enabled);
        let mut loaded = existing
            .as_ref()
            .map(|plugin| plugin.loaded)
            .unwrap_or(false);
        let mut resolved_version = version.to_string();

        if let Some(repo) = &self.state_repo {
            if let Err(error) = repo
                .upsert_profile_plugin(
                    plugin_id,
                    plugin_name,
                    "builtin",
                    version,
                    default_enabled,
                    required,
                )
                .await
            {
                tracing::warn!(
                    plugin = plugin_name,
                    error = %error,
                    "failed to upsert builtin runtime state"
                );
            }

            match repo.get_by_name(plugin_name).await {
                Ok(Some(state)) => {
                    resolved_plugin_id = state.plugin_id;
                    enabled = state.enabled;
                    loaded = state.loaded;
                    if !state.version.is_empty() {
                        resolved_version = state.version;
                    }
                }
                Ok(None) => {}
                Err(error) => {
                    tracing::warn!(
                        plugin = plugin_name,
                        error = %error,
                        "failed to read builtin runtime state"
                    );
                }
            }
        }

        self.plugin_info.write().await.insert(
            plugin_name.to_string(),
            PluginInfo {
                plugin_id: resolved_plugin_id,
                source_kind: "builtin".to_string(),
                name: plugin_name.to_string(),
                version: resolved_version,
                description: description.to_string(),
                enabled,
                loaded,
                permissions: PluginPermissionsView {
                    routes: permissions.routes,
                    commands: permissions.commands,
                    admin: permissions.admin,
                    database: db_permission_name(&permissions.database).to_string(),
                },
            },
        );
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
        self.register_profile_plugin_manifest(
            manifest,
            effective_permissions,
            plugin_id,
            kind,
            true,
            false,
        )
        .await;
    }

    pub async fn register_profile_plugin_manifest(
        &self,
        manifest: &PluginManifest,
        effective_permissions: &Permissions,
        plugin_id: &str,
        kind: PluginKind,
        default_enabled: bool,
        required: bool,
    ) {
        let plugin_name = manifest.plugin.name.clone();
        self.set_plugin_required(&plugin_name, required).await;
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
                .upsert_profile_plugin(
                    plugin_id,
                    &plugin_name,
                    &source_kind,
                    &manifest_version,
                    default_enabled,
                    required,
                )
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
        if let Some(repo) = &self.state_repo {
            if let Err(err) = repo.set_loaded(plugin_name, loaded).await {
                tracing::warn!(
                    "failed to persist plugin loaded state: plugin={} loaded={} error={}",
                    plugin_name,
                    loaded,
                    err
                );
            }
        }

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

    pub async fn is_plugin_required(&self, plugin_name: &str) -> bool {
        self.required_plugins.read().await.contains(plugin_name)
    }

    async fn set_plugin_required(&self, plugin_name: &str, required: bool) {
        let mut required_plugins = self.required_plugins.write().await;
        if required {
            required_plugins.insert(plugin_name.to_string());
        } else {
            required_plugins.remove(plugin_name);
        }
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
        self.vms.write().await.insert(
            plugin_name.to_string(),
            Arc::new(LuaRuntimeInstance::new(plugin_name, lua)),
        );
        self.mark_plugin_loaded(plugin_name, true).await;
    }

    pub async fn unregister_vm(&self, plugin_name: &str) {
        self.vms.write().await.remove(plugin_name);
        self.mark_plugin_loaded(plugin_name, false).await;
    }

    pub async fn prepare_owner_activation(
        &self,
        staged: StagedRegistrar,
    ) -> Result<PendingCapabilityCommit, RegistrationConflict> {
        self.capabilities.prepare_owner_replacement(staged).await
    }

    pub fn stage_owner_activation(&self, owner: PluginInstanceId) -> StagedRegistrar {
        self.capabilities.stage(owner)
    }

    pub fn stage_lua_activation(&self, owner: PluginInstanceId) -> StagedRegistrar {
        self.capabilities
            .stage_with_source(owner, RegistrationSource::Lua)
    }

    pub fn stage_builtin_activation(&self, owner: PluginInstanceId) -> StagedRegistrar {
        self.capabilities
            .stage_with_source(owner, RegistrationSource::Builtin)
    }

    pub async fn publish_lua_activation(
        &self,
        plugin_name: &str,
        pending: PendingCapabilityCommit,
        runtime: Arc<LuaRuntimeInstance>,
    ) {
        self.vms
            .write()
            .await
            .insert(plugin_name.to_string(), runtime);
        pending.publish().await;
        self.mark_plugin_loaded(plugin_name, true).await;
    }

    pub async fn has_vm(&self, plugin_name: &str) -> bool {
        self.vms.read().await.contains_key(plugin_name)
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
        let mut staged = self
            .capabilities
            .stage(PluginInstanceId::legacy(plugin_name));
        staged.register_http(
            HttpRouteSpec::new(method, path, plugin_name, handler_key)
                .with_policy(policy_key.map(ToOwned::to_owned))
                .with_public(is_public),
        );
        if let Err(err) = self.capabilities.commit(staged).await {
            tracing::warn!(
                "rejected legacy API registration: plugin={} method={} path={} error={}",
                plugin_name,
                method,
                path,
                err
            );
            return;
        }
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
        let mut staged = self
            .capabilities
            .stage(PluginInstanceId::legacy(plugin_name));
        staged.register_cli(
            CliCommandSpec::new(command_name, "", plugin_name, handler_key)
                .with_policy(policy_key.map(ToOwned::to_owned)),
        );
        if let Err(err) = self.capabilities.commit(staged).await {
            tracing::warn!(
                "rejected legacy CLI registration: plugin={} command={} error={}",
                plugin_name,
                command_name,
                err
            );
            return;
        }
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
        let mut staged = self
            .capabilities
            .stage(PluginInstanceId::legacy(plugin_name));
        staged.register_admin(
            AdminPageSpec::new(page_path, title, plugin_name, handler_key)
                .with_policy(policy_key.map(ToOwned::to_owned))
                .with_assets(assets.js.clone(), assets.css.clone()),
        );
        if let Err(err) = self.capabilities.commit(staged).await {
            tracing::warn!(
                "rejected legacy Admin registration: plugin={} path={} error={}",
                plugin_name,
                page_path,
                err
            );
            return;
        }
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
        let snapshot = self.capabilities.snapshot().await;
        let registration = snapshot.match_http(method, path)?;
        Some(
            self.dispatch_http_registration(&registration.value, path, dispatch_path, body)
                .await,
        )
    }

    pub async fn dispatch_http_registration(
        &self,
        registration: &HttpRouteSpec,
        path: &str,
        dispatch_path: &str,
        body: Option<Vec<u8>>,
    ) -> Result<String, String> {
        self.guard_plugin_enabled(&registration.plugin_name).await?;
        self.call_api_handler_with_dispatch(
            &registration.plugin_name,
            &registration.handler_key,
            path,
            dispatch_path,
            body,
            registration.lua_runtime.clone(),
        )
        .await
    }

    pub async fn dispatch_http_request_registration(
        &self,
        registration: &HttpRouteSpec,
        request: HttpRequest,
    ) -> Result<HttpResponse, String> {
        if let Some(handler) = &registration.rust_handler {
            self.guard_plugin_enabled(&registration.plugin_name).await?;
            return handler.call(request).await;
        }

        let response_body = self
            .dispatch_http_registration(
                registration,
                &request.path,
                &request.dispatch_path,
                request.body,
            )
            .await?;
        Ok(HttpResponse::from_plugin_body(response_body))
    }

    /// Return the optional policy key for an API route registration.
    pub async fn api_route_policy(&self, method: &str, path: &str) -> Option<String> {
        self.capabilities
            .snapshot()
            .await
            .match_http(method, path)
            .and_then(|registration| registration.value.policy_key.clone())
    }

    /// Return true when the API route was registered as public.
    pub async fn is_api_route_public(&self, method: &str, path: &str) -> bool {
        self.capabilities
            .snapshot()
            .await
            .match_http(method, path)
            .map(|registration| registration.value.is_public)
            .unwrap_or(false)
    }

    /// Call a CLI command handler with string args, returns the output.
    pub async fn call_cli_handler(
        &self,
        command_name: &str,
        args: &[String],
    ) -> Option<Result<String, String>> {
        let snapshot = self.capabilities.snapshot().await;
        let registration = snapshot.cli_command(command_name)?;
        let plugin_name = registration.value.plugin_name.clone();
        let handler_key = registration.value.handler_key.clone();
        let runtime = registration.value.lua_runtime.clone();
        if let Err(err) = self.guard_plugin_enabled(&plugin_name).await {
            return Some(Err(err));
        }
        Some(
            self.call_handler_with_args(&plugin_name, &handler_key, args, runtime)
                .await,
        )
    }

    /// Return the optional policy key for a CLI command registration.
    pub async fn cli_command_policy(&self, command_name: &str) -> Option<String> {
        self.capabilities
            .snapshot()
            .await
            .cli_command(command_name)
            .and_then(|registration| registration.value.policy_key.clone())
    }

    /// Call an admin page handler, returns HTML String.
    pub async fn call_admin_handler(&self, page_path: &str) -> Option<Result<String, String>> {
        let snapshot = self.capabilities.snapshot().await;
        let registration = snapshot.admin_page(page_path)?;
        Some(
            self.dispatch_admin_request_registration(
                &registration.value,
                HttpRequest::new("GET", page_path, page_path, None),
            )
            .await
            .and_then(|response| {
                String::from_utf8(response.body)
                    .map_err(|error| format!("admin response body is not UTF-8: {error}"))
            }),
        )
    }

    pub async fn dispatch_admin_registration(
        &self,
        registration: &AdminPageSpec,
    ) -> Result<String, String> {
        let response = self
            .dispatch_admin_request_registration(
                registration,
                HttpRequest::new("GET", &registration.path, &registration.path, None),
            )
            .await?;
        String::from_utf8(response.body)
            .map_err(|error| format!("admin response body is not UTF-8: {error}"))
    }

    pub async fn dispatch_admin_request_registration(
        &self,
        registration: &AdminPageSpec,
        request: HttpRequest,
    ) -> Result<HttpResponse, String> {
        self.guard_plugin_enabled(&registration.plugin_name).await?;
        if let Some(handler) = &registration.rust_handler {
            return handler.call(request).await;
        }
        let html = self
            .call_handler_no_args(
                &registration.plugin_name,
                &registration.handler_key,
                registration.lua_runtime.clone(),
            )
            .await?;
        Ok(HttpResponse::new(200, html).with_header("content-type", "text/html; charset=utf-8"))
    }

    /// Return the optional policy key for an admin page registration.
    pub async fn admin_page_policy(&self, page_path: &str) -> Option<String> {
        self.capabilities
            .snapshot()
            .await
            .admin_page(page_path)
            .and_then(|registration| registration.value.policy_key.clone())
    }

    /// List all registered API routes.
    pub async fn list_api_routes(&self) -> Vec<(String, String)> {
        self.capabilities
            .snapshot()
            .await
            .http_routes()
            .iter()
            .map(|registration| {
                (
                    registration.value.method.clone(),
                    registration.value.path.clone(),
                )
            })
            .collect()
    }

    /// List all registered CLI commands.
    pub async fn list_cli_commands(&self) -> Vec<String> {
        self.capabilities
            .snapshot()
            .await
            .cli_commands()
            .iter()
            .map(|registration| registration.value.name.clone())
            .collect()
    }

    /// List all registered admin pages.
    pub async fn list_admin_pages(&self) -> Vec<String> {
        self.capabilities
            .snapshot()
            .await
            .admin_pages()
            .iter()
            .map(|registration| registration.value.path.clone())
            .collect()
    }

    /// List all registered admin pages for a specific plugin.
    pub async fn list_admin_pages_for_plugin(&self, plugin_name: &str) -> Vec<PluginAdminPageInfo> {
        let snapshot = self.capabilities.snapshot().await;
        let mut pages = snapshot
            .admin_pages()
            .iter()
            .filter_map(|registration| {
                if registration.value.plugin_name == plugin_name {
                    Some(PluginAdminPageInfo {
                        plugin: registration.value.plugin_name.clone(),
                        path: registration.value.path.clone(),
                        title: registration.value.title.clone(),
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
        self.capabilities
            .snapshot()
            .await
            .admin_page(page_path)
            .map(|registration| PageResolvedAssets {
                js: registration.value.js.clone(),
                css: registration.value.css.clone(),
            })
    }

    pub async fn capability_snapshot(&self) -> Arc<CapabilitySnapshot> {
        self.capabilities.snapshot().await
    }

    pub fn capability_registry(&self) -> CapabilityRegistry {
        self.capabilities.clone()
    }

    pub async fn remove_owner_capabilities(&self, owner: &PluginInstanceId) {
        self.capabilities.remove_owner(owner).await;
    }

    /// Register plugin-scoped static assets root (`plugins/<name>/web/static`).
    pub async fn register_plugin_static_root(&self, plugin_name: &str, static_root: PathBuf) {
        let mut staged = self
            .capabilities
            .stage(PluginInstanceId::legacy(plugin_name));
        staged.register_static_root(StaticRootSpec::new(plugin_name, static_root));
        if let Err(err) = self.capabilities.commit(staged).await {
            tracing::warn!(plugin = plugin_name, error = %err, "failed to register legacy static root");
        }
    }

    /// List all registered plugin static roots sorted by plugin name.
    pub async fn list_plugin_static_roots(&self) -> Vec<(String, PathBuf)> {
        self.capabilities
            .snapshot()
            .await
            .static_roots()
            .iter()
            .map(|registration| {
                (
                    registration.value.plugin_id.clone(),
                    registration.value.root.clone(),
                )
            })
            .collect()
    }

    pub async fn set_plugin_enabled(
        &self,
        plugin_name: &str,
        enabled: bool,
        actor: Option<&str>,
        reason: Option<&str>,
    ) -> Result<PluginInfo, String> {
        if self.is_plugin_required(plugin_name).await {
            return Err(format!(
                "required_plugin_toggle_forbidden: plugin '{plugin_name}' must be changed through profile and restart"
            ));
        }
        let _runtime_guard = self.acquire_plugin_runtime_lock(plugin_name).await;
        self.set_plugin_enabled_intent(plugin_name, enabled, actor, reason)
            .await
    }

    pub(crate) async fn set_plugin_enabled_intent(
        &self,
        plugin_name: &str,
        enabled: bool,
        actor: Option<&str>,
        reason: Option<&str>,
    ) -> Result<PluginInfo, String> {
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
            let state = repo
                .set_enabled(plugin_name, enabled, actor, reason)
                .await?;
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

    pub async fn acquire_plugin_runtime_lock(&self, plugin_name: &str) -> OwnedMutexGuard<()> {
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
                                database: db_permission_name(&DatabasePermission::None).to_string(),
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
            return Err(format!(
                "plugin_disabled: plugin '{plugin_name}' is disabled"
            ));
        }
        Ok(())
    }

    async fn call_handler_no_args(
        &self,
        plugin_name: &str,
        handler_key: &str,
        runtime: Option<Arc<LuaRuntimeInstance>>,
    ) -> Result<String, String> {
        let runtime = self.resolve_lua_runtime(plugin_name, runtime).await?;
        let lua = runtime.lua();

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
        runtime: Option<Arc<LuaRuntimeInstance>>,
    ) -> Result<String, String> {
        let runtime = self.resolve_lua_runtime(plugin_name, runtime).await?;
        let lua = runtime.lua();

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
        runtime: Option<Arc<LuaRuntimeInstance>>,
    ) -> Result<String, String> {
        let runtime = self.resolve_lua_runtime(plugin_name, runtime).await?;
        let lua = runtime.lua();

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

    async fn resolve_lua_runtime(
        &self,
        plugin_name: &str,
        pinned: Option<Arc<LuaRuntimeInstance>>,
    ) -> Result<Arc<LuaRuntimeInstance>, String> {
        if let Some(runtime) = pinned {
            return Ok(runtime);
        }
        self.vms
            .read()
            .await
            .get(plugin_name)
            .cloned()
            .ok_or_else(|| format!("plugin '{plugin_name}' not loaded"))
    }

    fn get_handler_fn(&self, lua: &mlua::Lua, handler_key: &str) -> Result<mlua::Function, String> {
        let handlers: mlua::Table = match lua.globals().get::<mlua::Value>("app") {
            Ok(mlua::Value::Table(app)) => app
                .get("__handlers")
                .map_err(|e| format!("no app.__handlers table: {e}"))?,
            _ => {
                let sushi: mlua::Table = lua
                    .globals()
                    .get("sushi")
                    .map_err(|e| format!("no app or sushi global: {e}"))?;
                sushi
                    .get("__handlers")
                    .map_err(|e| format!("no sushi.__handlers table: {e}"))?
            }
        };

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
    use crate::runtime::{HttpHandler, HttpRequest, HttpResponse};
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
            .run_migrations(include_str!(
                "../../../../migrations/008_plugin_governance_v1.sql"
            ))
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
    async fn register_vm_persists_loaded_state_for_storage_backed_manager() {
        let storage = storage_with_governance_schema().await;
        let storage_trait: Arc<dyn Storage> = storage;
        let manager = PluginManager::new_with_storage(storage_trait);
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
        manager
            .register_plugin_manifest_with_permissions_and_identity(
                &manifest,
                &manifest.permissions,
                "third_party/example",
                PluginKind::ThirdParty,
            )
            .await;

        manager.register_vm("example", mlua::Lua::new()).await;

        let plugins = manager.list_plugins().await;
        assert_eq!(plugins.len(), 1);
        assert!(plugins[0].loaded);
    }

    #[tokio::test]
    async fn list_admin_pages_for_plugin_returns_titles() {
        let manager = PluginManager::new();
        manager
            .register_admin_handler(
                "/admin/kv-store/workspace",
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
                "/admin/kv-store/workspace",
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
            .admin_page_assets("/admin/kv-store/workspace")
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
    async fn legacy_registrations_are_projected_into_owned_capability_snapshot() {
        let manager = PluginManager::new();
        manager
            .register_api_handler_with_policy_and_public(
                "GET",
                "/api/notes",
                "notes",
                "handler::api",
                Some("api.notes.read"),
                false,
            )
            .await;
        manager
            .register_admin_handler_with_assets_and_policy(
                "/admin/notes",
                "notes",
                "Notes",
                "handler::admin",
                PageResolvedAssets {
                    js: vec!["/static/plugins/official/notes/notes.js".to_string()],
                    css: vec!["/static/plugins/official/notes/notes.css".to_string()],
                },
                Some("admin.notes.read"),
            )
            .await;
        manager
            .register_cli_handler_with_policy(
                "notes-list",
                "notes",
                "handler::cli",
                Some("cli.notes.read"),
            )
            .await;

        let snapshot = manager.capability_snapshot().await;
        assert_eq!(snapshot.http_routes().len(), 1);
        assert_eq!(snapshot.admin_pages().len(), 1);
        assert_eq!(snapshot.cli_commands().len(), 1);
        assert_eq!(snapshot.http_routes()[0].owner.as_str(), "legacy:notes");
        assert_eq!(
            snapshot.http_routes()[0].value.policy_key.as_deref(),
            Some("api.notes.read")
        );
        assert_eq!(
            snapshot.admin_pages()[0].value.js,
            vec!["/static/plugins/official/notes/notes.js".to_string()]
        );
        assert_eq!(
            snapshot.cli_commands()[0].value.policy_key.as_deref(),
            Some("cli.notes.read")
        );
    }

    #[tokio::test]
    async fn legacy_cross_owner_registration_does_not_override_existing_capability() {
        let manager = PluginManager::new();
        manager
            .register_api_handler_with_policy_and_public(
                "GET",
                "/api/notes",
                "notes",
                "handler::notes",
                Some("api.notes.read"),
                false,
            )
            .await;
        manager
            .register_api_handler_with_policy_and_public(
                "GET",
                "/api/notes",
                "cms",
                "handler::cms",
                Some("api.cms.read"),
                true,
            )
            .await;

        let snapshot = manager.capability_snapshot().await;
        assert_eq!(snapshot.http_routes().len(), 1);
        assert_eq!(snapshot.http_routes()[0].owner.as_str(), "legacy:notes");
        assert_eq!(
            snapshot.http_routes()[0].value.handler_key,
            "handler::notes"
        );
        assert_eq!(
            manager
                .api_route_policy("GET", "/api/notes")
                .await
                .as_deref(),
            Some("api.notes.read")
        );
        assert!(!manager.is_api_route_public("GET", "/api/notes").await);
    }

    #[tokio::test]
    async fn disabled_plugin_keeps_registrations_in_current_compatibility_phase() {
        let manager = PluginManager::new();
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
        manager
            .register_api_handler("GET", "/api/notes", "notes", "handler::notes")
            .await;

        manager
            .set_plugin_enabled("notes", false, Some("admin"), Some("characterization"))
            .await
            .expect("disable succeeds");

        assert_eq!(
            manager.list_api_routes().await,
            vec![("GET".to_string(), "/api/notes".to_string())]
        );
        assert!(!manager
            .plugin_runtime_enabled("notes")
            .await
            .expect("plugin remains known"));
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
    async fn in_flight_dispatch_keeps_snapshot_pinned_lua_runtime() {
        let manager = PluginManager::new();
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

        let entered = Arc::new(tokio::sync::Barrier::new(2));
        let release = Arc::new(tokio::sync::Barrier::new(2));
        let old_lua = mlua::Lua::new();
        let old_app = old_lua.create_table().unwrap();
        let old_handlers = old_lua.create_table().unwrap();
        old_app.set("__handlers", old_handlers.clone()).unwrap();
        old_lua.globals().set("app", old_app).unwrap();
        let old_handler = old_lua
            .create_async_function({
                let entered = Arc::clone(&entered);
                let release = Arc::clone(&release);
                move |_, _: mlua::Value| {
                    let entered = Arc::clone(&entered);
                    let release = Arc::clone(&release);
                    async move {
                        entered.wait().await;
                        release.wait().await;
                        Ok("old".to_string())
                    }
                }
            })
            .unwrap();
        old_handlers.set("handler", old_handler).unwrap();
        let old_runtime = Arc::new(LuaRuntimeInstance::new("notes", old_lua));
        let mut old_staged =
            manager.stage_owner_activation(PluginInstanceId::new("lua:notes").unwrap());
        old_staged.register_http(
            HttpRouteSpec::new("GET", "/api/notes", "notes", "handler")
                .with_lua_runtime(Arc::clone(&old_runtime)),
        );
        let old_pending = manager.prepare_owner_activation(old_staged).await.unwrap();
        manager
            .publish_lua_activation("notes", old_pending, old_runtime)
            .await;

        let in_flight_manager = manager.clone();
        let in_flight = tokio::spawn(async move {
            in_flight_manager
                .call_api_handler("GET", "/api/notes", None)
                .await
                .unwrap()
                .unwrap()
        });
        entered.wait().await;

        let new_lua = mlua::Lua::new();
        let new_app = new_lua.create_table().unwrap();
        let new_handlers = new_lua.create_table().unwrap();
        new_app.set("__handlers", new_handlers.clone()).unwrap();
        new_lua.globals().set("app", new_app).unwrap();
        new_handlers
            .set(
                "handler",
                new_lua
                    .create_function(|_, _: mlua::Value| Ok("new".to_string()))
                    .unwrap(),
            )
            .unwrap();
        let new_runtime = Arc::new(LuaRuntimeInstance::new("notes", new_lua));
        let mut new_staged =
            manager.stage_owner_activation(PluginInstanceId::new("lua:notes").unwrap());
        new_staged.register_http(
            HttpRouteSpec::new("GET", "/api/notes", "notes", "handler")
                .with_lua_runtime(Arc::clone(&new_runtime)),
        );
        let new_pending = manager.prepare_owner_activation(new_staged).await.unwrap();
        manager
            .publish_lua_activation("notes", new_pending, new_runtime)
            .await;

        assert_eq!(
            manager
                .call_api_handler("GET", "/api/notes", None)
                .await
                .unwrap()
                .unwrap(),
            "new"
        );
        release.wait().await;
        assert_eq!(in_flight.await.unwrap(), "old");
    }

    #[tokio::test]
    async fn in_flight_dispatch_completes_after_owner_deactivation() {
        let manager = PluginManager::new();
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

        let entered = Arc::new(tokio::sync::Barrier::new(2));
        let release = Arc::new(tokio::sync::Barrier::new(2));
        let lua = mlua::Lua::new();
        let app = lua.create_table().unwrap();
        let handlers = lua.create_table().unwrap();
        app.set("__handlers", handlers.clone()).unwrap();
        lua.globals().set("app", app).unwrap();
        handlers
            .set(
                "handler",
                lua.create_async_function({
                    let entered = Arc::clone(&entered);
                    let release = Arc::clone(&release);
                    move |_, _: mlua::Value| {
                        let entered = Arc::clone(&entered);
                        let release = Arc::clone(&release);
                        async move {
                            entered.wait().await;
                            release.wait().await;
                            Ok("completed".to_string())
                        }
                    }
                })
                .unwrap(),
            )
            .unwrap();
        let runtime = Arc::new(LuaRuntimeInstance::new("notes", lua));
        let owner = PluginInstanceId::new("lua:notes").unwrap();
        let mut staged = manager.stage_owner_activation(owner.clone());
        staged.register_http(
            HttpRouteSpec::new("GET", "/api/notes", "notes", "handler")
                .with_lua_runtime(Arc::clone(&runtime)),
        );
        let pending = manager.prepare_owner_activation(staged).await.unwrap();
        manager
            .publish_lua_activation("notes", pending, runtime)
            .await;

        let in_flight_manager = manager.clone();
        let in_flight = tokio::spawn(async move {
            in_flight_manager
                .call_api_handler("GET", "/api/notes", None)
                .await
                .unwrap()
                .unwrap()
        });
        entered.wait().await;

        manager.remove_owner_capabilities(&owner).await;
        manager.unregister_vm("notes").await;
        assert!(manager
            .call_api_handler("GET", "/api/notes", None)
            .await
            .is_none());

        release.wait().await;
        assert_eq!(in_flight.await.unwrap(), "completed");
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

    #[tokio::test]
    async fn rust_http_handler_uses_shared_dispatch_and_enable_guard() {
        let manager = PluginManager::new();
        let manifest = PluginManifest {
            plugin: PluginMeta {
                name: "builtin-notes".to_string(),
                version: "0.1.0".to_string(),
                description: "Builtin notes".to_string(),
                entry: String::new(),
            },
            permissions: Permissions::default(),
            policies: PluginPoliciesConfig::default(),
            admin: None,
            file_browser: None,
        };
        manager.register_plugin_manifest(&manifest).await;

        let mut staged =
            manager.stage_owner_activation(PluginInstanceId::new("builtin.notes").unwrap());
        staged.register_http(
            HttpRouteSpec::new(
                "POST",
                "/api/builtin-notes",
                "builtin-notes",
                "rust::create",
            )
            .with_rust_handler(HttpHandler::new(|request| async move {
                Ok(HttpResponse::new(201, request.body.unwrap_or_default())
                    .with_header("content-type", "application/octet-stream"))
            })),
        );
        manager
            .prepare_owner_activation(staged)
            .await
            .unwrap()
            .publish()
            .await;

        let snapshot = manager.capability_snapshot().await;
        let registration = snapshot
            .match_http("POST", "/api/builtin-notes")
            .unwrap()
            .value
            .clone();
        let response = manager
            .dispatch_http_request_registration(
                &registration,
                HttpRequest::new(
                    "POST",
                    "/api/builtin-notes",
                    "/api/builtin-notes",
                    Some(vec![0xff, 0x00]),
                ),
            )
            .await
            .unwrap();
        assert_eq!(response.status, 201);
        assert_eq!(response.body, vec![0xff, 0x00]);

        manager
            .set_plugin_enabled("builtin-notes", false, None, None)
            .await
            .unwrap();
        let error = manager
            .dispatch_http_request_registration(
                &registration,
                HttpRequest::new("POST", "/api/builtin-notes", "/api/builtin-notes", None),
            )
            .await
            .expect_err("disabled Rust handler must fail closed");
        assert!(error.starts_with("plugin_disabled:"), "error: {error}");
    }

    #[tokio::test]
    async fn rust_admin_handler_uses_shared_dispatch_and_enable_guard() {
        let manager = PluginManager::new();
        let manifest = PluginManifest {
            plugin: PluginMeta {
                name: "builtin-admin".to_string(),
                version: "0.1.0".to_string(),
                description: "Builtin admin".to_string(),
                entry: String::new(),
            },
            permissions: Permissions::default(),
            policies: PluginPoliciesConfig::default(),
            admin: None,
            file_browser: None,
        };
        manager.register_plugin_manifest(&manifest).await;

        let mut staged =
            manager.stage_owner_activation(PluginInstanceId::new("builtin.admin").unwrap());
        staged.register_admin(
            AdminPageSpec::new("/admin/builtin", "Builtin", "builtin-admin", "rust::admin")
                .with_rust_handler(HttpHandler::new(|request| async move {
                    assert_eq!(request.path, "/admin/builtin");
                    Ok(HttpResponse::new(200, "<main>builtin</main>")
                        .with_header("content-type", "text/html; charset=utf-8"))
                })),
        );
        manager
            .prepare_owner_activation(staged)
            .await
            .unwrap()
            .publish()
            .await;

        let snapshot = manager.capability_snapshot().await;
        let registration = snapshot.admin_page("/admin/builtin").unwrap().value.clone();
        let response = manager
            .dispatch_admin_request_registration(
                &registration,
                HttpRequest::new("GET", "/admin/builtin", "/admin/builtin", None),
            )
            .await
            .unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(response.body, b"<main>builtin</main>");
        assert_eq!(response.headers[0].1, "text/html; charset=utf-8");

        manager
            .set_plugin_enabled("builtin-admin", false, None, None)
            .await
            .unwrap();
        let error = manager
            .dispatch_admin_request_registration(
                &registration,
                HttpRequest::new("GET", "/admin/builtin", "/admin/builtin", None),
            )
            .await
            .expect_err("disabled Rust admin handler must fail closed");
        assert!(error.starts_with("plugin_disabled:"), "error: {error}");
    }
}
