use super::{HttpHandler, LuaRuntimeInstance, PluginInstanceId, RegistrationId};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use thiserror::Error;
use tokio::sync::{Mutex, OwnedMutexGuard};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum HttpSurface {
    Api,
    Admin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RegistrationSource {
    Builtin,
    Lua,
    Legacy,
}

impl RegistrationSource {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Builtin => "builtin",
            Self::Lua => "lua",
            Self::Legacy => "legacy",
        }
    }
}

impl std::fmt::Display for RegistrationSource {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy)]
struct ReservedHttpRoute {
    surface: HttpSurface,
    method: &'static str,
    path: &'static str,
}

const HOST_RESERVED_HTTP_ROUTES: &[ReservedHttpRoute] = &[
    ReservedHttpRoute {
        surface: HttpSurface::Api,
        method: "GET",
        path: "/",
    },
    ReservedHttpRoute {
        surface: HttpSurface::Api,
        method: "GET",
        path: "/favicon.ico",
    },
    ReservedHttpRoute {
        surface: HttpSurface::Api,
        method: "GET",
        path: "/favicon.svg",
    },
    ReservedHttpRoute {
        surface: HttpSurface::Api,
        method: "GET",
        path: "/index.html",
    },
    ReservedHttpRoute {
        surface: HttpSurface::Api,
        method: "GET",
        path: "/admin-login",
    },
    ReservedHttpRoute {
        surface: HttpSurface::Api,
        method: "POST",
        path: "/admin-login",
    },
    ReservedHttpRoute {
        surface: HttpSurface::Api,
        method: "POST",
        path: "/api/auth/login",
    },
    ReservedHttpRoute {
        surface: HttpSurface::Api,
        method: "GET",
        path: "/api/auth/me",
    },
    ReservedHttpRoute {
        surface: HttpSurface::Api,
        method: "POST",
        path: "/api/auth/refresh",
    },
    ReservedHttpRoute {
        surface: HttpSurface::Api,
        method: "GET",
        path: "/api/users",
    },
    ReservedHttpRoute {
        surface: HttpSurface::Api,
        method: "POST",
        path: "/api/users",
    },
    ReservedHttpRoute {
        surface: HttpSurface::Api,
        method: "DELETE",
        path: "/api/users/{id}",
    },
    ReservedHttpRoute {
        surface: HttpSurface::Admin,
        method: "GET",
        path: "/admin",
    },
    ReservedHttpRoute {
        surface: HttpSurface::Admin,
        method: "GET",
        path: "/admin/",
    },
    ReservedHttpRoute {
        surface: HttpSurface::Admin,
        method: "GET",
        path: "/admin/workspace/{*module}",
    },
    ReservedHttpRoute {
        surface: HttpSurface::Admin,
        method: "GET",
        path: "/admin/api/workspace/assets",
    },
    ReservedHttpRoute {
        surface: HttpSurface::Admin,
        method: "GET",
        path: "/admin/plugins",
    },
    ReservedHttpRoute {
        surface: HttpSurface::Admin,
        method: "GET",
        path: "/admin/plugins/{plugin}",
    },
    ReservedHttpRoute {
        surface: HttpSurface::Admin,
        method: "GET",
        path: "/admin/users",
    },
    ReservedHttpRoute {
        surface: HttpSurface::Admin,
        method: "GET",
        path: "/admin/roles",
    },
    ReservedHttpRoute {
        surface: HttpSurface::Admin,
        method: "GET",
        path: "/admin/permissions",
    },
    ReservedHttpRoute {
        surface: HttpSurface::Admin,
        method: "GET",
        path: "/admin/config",
    },
    ReservedHttpRoute {
        surface: HttpSurface::Admin,
        method: "GET",
        path: "/admin/api/config",
    },
    ReservedHttpRoute {
        surface: HttpSurface::Admin,
        method: "GET",
        path: "/admin/logs",
    },
    ReservedHttpRoute {
        surface: HttpSurface::Admin,
        method: "GET",
        path: "/admin/api/logs",
    },
    ReservedHttpRoute {
        surface: HttpSurface::Admin,
        method: "GET",
        path: "/admin/menus",
    },
    ReservedHttpRoute {
        surface: HttpSurface::Admin,
        method: "GET",
        path: "/admin/api/menu",
    },
    ReservedHttpRoute {
        surface: HttpSurface::Admin,
        method: "POST",
        path: "/admin/api/menu",
    },
    ReservedHttpRoute {
        surface: HttpSurface::Admin,
        method: "PUT",
        path: "/admin/api/menu/{id}",
    },
    ReservedHttpRoute {
        surface: HttpSurface::Admin,
        method: "DELETE",
        path: "/admin/api/menu/{id}",
    },
    ReservedHttpRoute {
        surface: HttpSurface::Admin,
        method: "GET",
        path: "/admin/partials/users/table",
    },
    ReservedHttpRoute {
        surface: HttpSurface::Admin,
        method: "POST",
        path: "/admin/partials/users/create",
    },
    ReservedHttpRoute {
        surface: HttpSurface::Admin,
        method: "DELETE",
        path: "/admin/partials/users/{id}",
    },
    ReservedHttpRoute {
        surface: HttpSurface::Admin,
        method: "GET",
        path: "/admin/partials/roles/table",
    },
    ReservedHttpRoute {
        surface: HttpSurface::Admin,
        method: "POST",
        path: "/admin/partials/roles/create",
    },
    ReservedHttpRoute {
        surface: HttpSurface::Admin,
        method: "POST",
        path: "/admin/partials/roles/{id}/update",
    },
    ReservedHttpRoute {
        surface: HttpSurface::Admin,
        method: "DELETE",
        path: "/admin/partials/roles/{id}",
    },
    ReservedHttpRoute {
        surface: HttpSurface::Admin,
        method: "GET",
        path: "/admin/partials/permissions/table",
    },
    ReservedHttpRoute {
        surface: HttpSurface::Admin,
        method: "POST",
        path: "/admin/partials/permissions/create",
    },
    ReservedHttpRoute {
        surface: HttpSurface::Admin,
        method: "POST",
        path: "/admin/partials/permissions/{id}/update",
    },
    ReservedHttpRoute {
        surface: HttpSurface::Admin,
        method: "DELETE",
        path: "/admin/partials/permissions/{id}",
    },
    ReservedHttpRoute {
        surface: HttpSurface::Admin,
        method: "GET",
        path: "/admin/partials/plugins/table",
    },
    ReservedHttpRoute {
        surface: HttpSurface::Admin,
        method: "GET",
        path: "/admin/partials/menus/table",
    },
    ReservedHttpRoute {
        surface: HttpSurface::Admin,
        method: "POST",
        path: "/admin/partials/menus/create",
    },
    ReservedHttpRoute {
        surface: HttpSurface::Admin,
        method: "POST",
        path: "/admin/partials/menus/{id}/update",
    },
    ReservedHttpRoute {
        surface: HttpSurface::Admin,
        method: "DELETE",
        path: "/admin/partials/menus/{id}",
    },
    ReservedHttpRoute {
        surface: HttpSurface::Admin,
        method: "GET",
        path: "/admin/partials/roles/{id}/permissions/form",
    },
    ReservedHttpRoute {
        surface: HttpSurface::Admin,
        method: "POST",
        path: "/admin/partials/roles/{id}/permissions",
    },
    ReservedHttpRoute {
        surface: HttpSurface::Admin,
        method: "GET",
        path: "/admin/api/plugins",
    },
    ReservedHttpRoute {
        surface: HttpSurface::Admin,
        method: "GET",
        path: "/admin/api/plugins/{plugin}/pages",
    },
    ReservedHttpRoute {
        surface: HttpSurface::Admin,
        method: "PATCH",
        path: "/admin/api/plugins/{plugin}/state",
    },
];

impl HttpSurface {
    pub fn from_path(path: &str) -> Self {
        if path == "/admin" || path.starts_with("/admin/") {
            Self::Admin
        } else {
            Self::Api
        }
    }

    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Api => "api",
            Self::Admin => "admin",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRouteSpec {
    pub method: String,
    pub path: String,
    pub surface: HttpSurface,
    pub plugin_name: String,
    pub handler_key: String,
    pub policy_key: Option<String>,
    pub is_public: bool,
    pub rust_handler: Option<HttpHandler>,
    pub lua_runtime: Option<Arc<LuaRuntimeInstance>>,
}

impl HttpRouteSpec {
    pub fn new(
        method: impl Into<String>,
        path: impl Into<String>,
        plugin_name: impl Into<String>,
        handler_key: impl Into<String>,
    ) -> Self {
        let path = path.into();
        Self {
            method: method.into().to_uppercase(),
            surface: HttpSurface::from_path(&path),
            path,
            plugin_name: plugin_name.into(),
            handler_key: handler_key.into(),
            policy_key: None,
            is_public: false,
            rust_handler: None,
            lua_runtime: None,
        }
    }

    pub fn with_surface(mut self, surface: HttpSurface) -> Self {
        self.surface = surface;
        self
    }

    pub fn with_policy(mut self, policy_key: Option<String>) -> Self {
        self.policy_key = policy_key;
        self
    }

    pub fn with_public(mut self, is_public: bool) -> Self {
        self.is_public = is_public;
        self
    }

    pub fn with_rust_handler(mut self, handler: HttpHandler) -> Self {
        self.rust_handler = Some(handler);
        self
    }

    pub fn with_lua_runtime(mut self, runtime: Arc<LuaRuntimeInstance>) -> Self {
        self.lua_runtime = Some(runtime);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminPageSpec {
    pub path: String,
    pub title: String,
    pub plugin_name: String,
    pub handler_key: String,
    pub policy_key: Option<String>,
    pub js: Vec<String>,
    pub css: Vec<String>,
    pub rust_handler: Option<HttpHandler>,
    pub lua_runtime: Option<Arc<LuaRuntimeInstance>>,
}

impl AdminPageSpec {
    pub fn new(
        path: impl Into<String>,
        title: impl Into<String>,
        plugin_name: impl Into<String>,
        handler_key: impl Into<String>,
    ) -> Self {
        Self {
            path: path.into(),
            title: title.into(),
            plugin_name: plugin_name.into(),
            handler_key: handler_key.into(),
            policy_key: None,
            js: Vec::new(),
            css: Vec::new(),
            rust_handler: None,
            lua_runtime: None,
        }
    }

    pub fn with_policy(mut self, policy_key: Option<String>) -> Self {
        self.policy_key = policy_key;
        self
    }

    pub fn with_assets(mut self, js: Vec<String>, css: Vec<String>) -> Self {
        self.js = js;
        self.css = css;
        self
    }

    pub fn with_rust_handler(mut self, handler: HttpHandler) -> Self {
        self.rust_handler = Some(handler);
        self
    }

    pub fn with_lua_runtime(mut self, runtime: Arc<LuaRuntimeInstance>) -> Self {
        self.lua_runtime = Some(runtime);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliCommandSpec {
    pub name: String,
    pub description: String,
    pub plugin_name: String,
    pub handler_key: String,
    pub policy_key: Option<String>,
    pub lua_runtime: Option<Arc<LuaRuntimeInstance>>,
}

impl CliCommandSpec {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        plugin_name: impl Into<String>,
        handler_key: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            plugin_name: plugin_name.into(),
            handler_key: handler_key.into(),
            policy_key: None,
            lua_runtime: None,
        }
    }

    pub fn with_policy(mut self, policy_key: Option<String>) -> Self {
        self.policy_key = policy_key;
        self
    }

    pub fn with_lua_runtime(mut self, runtime: Arc<LuaRuntimeInstance>) -> Self {
        self.lua_runtime = Some(runtime);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuContributionSpec {
    pub id: String,
    pub label: String,
    pub icon: Option<String>,
    pub position: i64,
    pub parent_id: Option<String>,
    pub route: Option<String>,
    pub policy_key: Option<String>,
}

impl MenuContributionSpec {
    pub fn new(id: impl Into<String>, label: impl Into<String>, position: i64) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            position,
            parent_id: None,
            route: None,
            policy_key: None,
        }
    }

    pub fn with_icon(mut self, icon: Option<String>) -> Self {
        self.icon = icon;
        self
    }

    pub fn with_parent(mut self, parent_id: Option<String>) -> Self {
        self.parent_id = parent_id;
        self
    }

    pub fn with_route(mut self, route: Option<String>) -> Self {
        self.route = route;
        self
    }

    pub fn with_policy(mut self, policy_key: Option<String>) -> Self {
        self.policy_key = policy_key;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateRootSpec {
    pub plugin_id: String,
    pub root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticRootSpec {
    pub plugin_id: String,
    pub root: PathBuf,
}

impl StaticRootSpec {
    pub fn new(plugin_id: impl Into<String>, root: PathBuf) -> Self {
        Self {
            plugin_id: plugin_id.into(),
            root,
        }
    }
}

type EventCallback = Arc<dyn Fn(Value) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

static NEXT_EVENT_SUBSCRIPTION_ID: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(1);

#[derive(Clone)]
pub struct EventSubscriptionSpec {
    pub subscription_id: u64,
    pub event: String,
    callback: EventCallback,
    pub lua_runtime: Option<Arc<LuaRuntimeInstance>>,
}

impl EventSubscriptionSpec {
    pub fn new<F, Fut>(event: impl Into<String>, callback: F) -> Self
    where
        F: Fn(Value) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        Self {
            subscription_id: NEXT_EVENT_SUBSCRIPTION_ID
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            event: event.into(),
            callback: Arc::new(move |data| Box::pin(callback(data))),
            lua_runtime: None,
        }
    }

    pub fn with_lua_runtime(mut self, runtime: Arc<LuaRuntimeInstance>) -> Self {
        self.lua_runtime = Some(runtime);
        self
    }

    pub async fn call(&self, data: Value) {
        (self.callback)(data).await;
    }
}

impl std::fmt::Debug for EventSubscriptionSpec {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EventSubscriptionSpec")
            .field("subscription_id", &self.subscription_id)
            .field("event", &self.event)
            .field(
                "lua_runtime_id",
                &self.lua_runtime.as_ref().map(|runtime| runtime.id()),
            )
            .finish()
    }
}

impl PartialEq for EventSubscriptionSpec {
    fn eq(&self, other: &Self) -> bool {
        self.subscription_id == other.subscription_id
            && self.event == other.event
            && self.lua_runtime == other.lua_runtime
    }
}

impl Eq for EventSubscriptionSpec {}

impl TemplateRootSpec {
    pub fn new(plugin_id: impl Into<String>, root: PathBuf) -> Self {
        Self {
            plugin_id: plugin_id.into(),
            root,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedRegistration<T> {
    pub id: RegistrationId,
    pub owner: PluginInstanceId,
    pub source: RegistrationSource,
    pub value: T,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CapabilitySnapshot {
    http_routes: Vec<OwnedRegistration<HttpRouteSpec>>,
    admin_pages: Vec<OwnedRegistration<AdminPageSpec>>,
    cli_commands: Vec<OwnedRegistration<CliCommandSpec>>,
    menu_contributions: Vec<OwnedRegistration<MenuContributionSpec>>,
    template_roots: Vec<OwnedRegistration<TemplateRootSpec>>,
    static_roots: Vec<OwnedRegistration<StaticRootSpec>>,
    event_subscriptions: Vec<OwnedRegistration<EventSubscriptionSpec>>,
}

impl CapabilitySnapshot {
    pub fn http_routes(&self) -> &[OwnedRegistration<HttpRouteSpec>] {
        &self.http_routes
    }

    pub fn admin_pages(&self) -> &[OwnedRegistration<AdminPageSpec>] {
        &self.admin_pages
    }

    pub fn cli_commands(&self) -> &[OwnedRegistration<CliCommandSpec>] {
        &self.cli_commands
    }

    pub fn menu_contributions(&self) -> &[OwnedRegistration<MenuContributionSpec>] {
        &self.menu_contributions
    }

    pub fn template_roots(&self) -> &[OwnedRegistration<TemplateRootSpec>] {
        &self.template_roots
    }

    pub fn static_roots(&self) -> &[OwnedRegistration<StaticRootSpec>] {
        &self.static_roots
    }

    pub fn event_subscriptions(&self) -> &[OwnedRegistration<EventSubscriptionSpec>] {
        &self.event_subscriptions
    }

    pub fn match_http(
        &self,
        method: &str,
        path: &str,
    ) -> Option<&OwnedRegistration<HttpRouteSpec>> {
        self.match_http_on(HttpSurface::from_path(path), method, path)
    }

    pub fn match_http_on(
        &self,
        surface: HttpSurface,
        method: &str,
        path: &str,
    ) -> Option<&OwnedRegistration<HttpRouteSpec>> {
        let method = method.to_uppercase();
        self.http_routes
            .iter()
            .filter(|entry| entry.value.surface == surface && entry.value.method == method)
            .filter_map(|entry| {
                http_path_match_score(&entry.value.path, path).map(|score| (score, entry))
            })
            .max_by_key(|(score, entry)| (*score, std::cmp::Reverse(entry.value.path.clone())))
            .map(|(_, entry)| entry)
    }

    pub fn admin_page(&self, path: &str) -> Option<&OwnedRegistration<AdminPageSpec>> {
        self.admin_pages
            .iter()
            .filter_map(|entry| {
                http_path_match_score(&entry.value.path, path).map(|score| (score, entry))
            })
            .max_by_key(|(score, entry)| (*score, std::cmp::Reverse(entry.value.path.clone())))
            .map(|(_, entry)| entry)
    }

    pub fn cli_command(&self, name: &str) -> Option<&OwnedRegistration<CliCommandSpec>> {
        self.cli_commands
            .iter()
            .find(|entry| entry.value.name == name)
    }

    pub fn inspect(&self) -> Vec<CapabilityInspectionEntry> {
        let mut entries = Vec::with_capacity(
            self.http_routes.len()
                + self.admin_pages.len()
                + self.cli_commands.len()
                + self.menu_contributions.len()
                + self.template_roots.len()
                + self.static_roots.len()
                + self.event_subscriptions.len(),
        );
        entries.extend(
            self.http_routes
                .iter()
                .map(|registration| CapabilityInspectionEntry {
                    key: format!(
                        "http:{}:{}:{}",
                        registration.value.surface.as_str(),
                        registration.value.method,
                        registration.value.path
                    ),
                    owner: registration.owner.clone(),
                    source: registration.source,
                    registration_id: registration.id,
                }),
        );
        entries.extend(
            self.admin_pages
                .iter()
                .map(|registration| CapabilityInspectionEntry {
                    key: format!("admin:{}", registration.value.path),
                    owner: registration.owner.clone(),
                    source: registration.source,
                    registration_id: registration.id,
                }),
        );
        entries.extend(
            self.cli_commands
                .iter()
                .map(|registration| CapabilityInspectionEntry {
                    key: format!("cli:{}", registration.value.name),
                    owner: registration.owner.clone(),
                    source: registration.source,
                    registration_id: registration.id,
                }),
        );
        entries.extend(self.menu_contributions.iter().map(|registration| {
            CapabilityInspectionEntry {
                key: format!("menu:{}", registration.value.id),
                owner: registration.owner.clone(),
                source: registration.source,
                registration_id: registration.id,
            }
        }));
        entries.extend(
            self.template_roots
                .iter()
                .map(|registration| CapabilityInspectionEntry {
                    key: format!("template:{}", registration.value.plugin_id),
                    owner: registration.owner.clone(),
                    source: registration.source,
                    registration_id: registration.id,
                }),
        );
        entries.extend(
            self.static_roots
                .iter()
                .map(|registration| CapabilityInspectionEntry {
                    key: format!("static:{}", registration.value.plugin_id),
                    owner: registration.owner.clone(),
                    source: registration.source,
                    registration_id: registration.id,
                }),
        );
        entries.extend(self.event_subscriptions.iter().map(|registration| {
            CapabilityInspectionEntry {
                key: format!("event:{}", registration.value.event),
                owner: registration.owner.clone(),
                source: registration.source,
                registration_id: registration.id,
            }
        }));
        entries.sort_by(|left, right| left.key.cmp(&right.key));
        entries
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityInspectionEntry {
    pub key: String,
    pub owner: PluginInstanceId,
    pub source: RegistrationSource,
    pub registration_id: RegistrationId,
}

#[derive(Debug, Clone, Default)]
pub struct CapabilityRegistry {
    current: Arc<RwLock<Arc<CapabilitySnapshot>>>,
    commit_lock: Arc<Mutex<()>>,
}

impl CapabilityRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn stage(&self, owner: PluginInstanceId) -> StagedRegistrar {
        self.stage_with_source(owner, RegistrationSource::Legacy)
    }

    pub fn stage_with_source(
        &self,
        owner: PluginInstanceId,
        source: RegistrationSource,
    ) -> StagedRegistrar {
        StagedRegistrar::new(owner, source)
    }

    pub async fn snapshot(&self) -> Arc<CapabilitySnapshot> {
        self.snapshot_sync()
    }

    pub fn snapshot_sync(&self) -> Arc<CapabilitySnapshot> {
        Arc::clone(
            &self
                .current
                .read()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
        )
    }

    pub async fn commit(&self, staged: StagedRegistrar) -> Result<(), RegistrationConflict> {
        self.prepare(staged).await?.publish().await;
        Ok(())
    }

    pub async fn prepare(
        &self,
        staged: StagedRegistrar,
    ) -> Result<PendingCapabilityCommit, RegistrationConflict> {
        self.prepare_with_mode(staged, CommitMode::Merge).await
    }

    pub async fn prepare_owner_replacement(
        &self,
        staged: StagedRegistrar,
    ) -> Result<PendingCapabilityCommit, RegistrationConflict> {
        self.prepare_with_mode(staged, CommitMode::ReplaceOwner)
            .await
    }

    pub async fn remove_owner(&self, owner: &PluginInstanceId) {
        let _commit_guard = Arc::clone(&self.commit_lock).lock_owned().await;
        let mut next = self.snapshot_sync().as_ref().clone();
        next.http_routes.retain(|entry| &entry.owner != owner);
        next.admin_pages.retain(|entry| &entry.owner != owner);
        next.cli_commands.retain(|entry| &entry.owner != owner);
        next.menu_contributions
            .retain(|entry| &entry.owner != owner);
        next.template_roots.retain(|entry| &entry.owner != owner);
        next.static_roots.retain(|entry| &entry.owner != owner);
        next.event_subscriptions
            .retain(|entry| &entry.owner != owner);
        *self
            .current
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Arc::new(next);
    }

    pub async fn remove_event_subscription(&self, subscription_id: u64) {
        let _commit_guard = Arc::clone(&self.commit_lock).lock_owned().await;
        let mut next = self.snapshot_sync().as_ref().clone();
        next.event_subscriptions
            .retain(|entry| entry.value.subscription_id != subscription_id);
        *self
            .current
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Arc::new(next);
    }

    pub async fn remove_event_subscriptions_by_owner(&self, owner: &PluginInstanceId) {
        let _commit_guard = Arc::clone(&self.commit_lock).lock_owned().await;
        let mut next = self.snapshot_sync().as_ref().clone();
        next.event_subscriptions
            .retain(|entry| &entry.owner != owner);
        *self
            .current
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Arc::new(next);
    }

    async fn prepare_with_mode(
        &self,
        staged: StagedRegistrar,
        mode: CommitMode,
    ) -> Result<PendingCapabilityCommit, RegistrationConflict> {
        let commit_guard = Arc::clone(&self.commit_lock).lock_owned().await;
        let current = self.snapshot().await;
        let next = build_committed_snapshot(current.as_ref(), staged, mode)?;
        Ok(PendingCapabilityCommit {
            current: Arc::clone(&self.current),
            commit_guard,
            next,
        })
    }
}

pub struct PendingCapabilityCommit {
    current: Arc<RwLock<Arc<CapabilitySnapshot>>>,
    commit_guard: OwnedMutexGuard<()>,
    next: CapabilitySnapshot,
}

impl PendingCapabilityCommit {
    pub async fn publish(self) {
        let Self {
            current,
            commit_guard,
            next,
        } = self;
        *current
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Arc::new(next);
        drop(commit_guard);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommitMode {
    Merge,
    ReplaceOwner,
}

#[derive(Debug)]
pub struct StagedRegistrar {
    owner: PluginInstanceId,
    source: RegistrationSource,
    http_routes: Vec<HttpRouteSpec>,
    admin_pages: Vec<AdminPageSpec>,
    cli_commands: Vec<CliCommandSpec>,
    menu_contributions: Vec<MenuContributionSpec>,
    template_roots: Vec<TemplateRootSpec>,
    static_roots: Vec<StaticRootSpec>,
    event_subscriptions: Vec<EventSubscriptionSpec>,
}

impl StagedRegistrar {
    fn new(owner: PluginInstanceId, source: RegistrationSource) -> Self {
        Self {
            owner,
            source,
            http_routes: Vec::new(),
            admin_pages: Vec::new(),
            cli_commands: Vec::new(),
            menu_contributions: Vec::new(),
            template_roots: Vec::new(),
            static_roots: Vec::new(),
            event_subscriptions: Vec::new(),
        }
    }

    pub fn owner(&self) -> &PluginInstanceId {
        &self.owner
    }

    pub fn source(&self) -> RegistrationSource {
        self.source
    }

    pub fn register_http(&mut self, spec: HttpRouteSpec) {
        self.http_routes.push(spec);
    }

    pub fn register_admin(&mut self, spec: AdminPageSpec) {
        self.admin_pages.push(spec);
    }

    pub fn register_cli(&mut self, spec: CliCommandSpec) {
        self.cli_commands.push(spec);
    }

    pub fn register_menu(&mut self, spec: MenuContributionSpec) {
        self.menu_contributions.push(spec);
    }

    pub fn register_template_root(&mut self, spec: TemplateRootSpec) {
        self.template_roots.push(spec);
    }

    pub fn register_static_root(&mut self, spec: StaticRootSpec) {
        self.static_roots.push(spec);
    }

    pub fn register_event_subscription(&mut self, spec: EventSubscriptionSpec) {
        self.event_subscriptions.push(spec);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RegistrationConflict {
    #[error(
        "HTTP route conflict for {method} {path}: existing owner {existing_owner}, incoming owner {incoming_owner}"
    )]
    HttpRoute {
        method: String,
        path: String,
        existing_owner: PluginInstanceId,
        incoming_owner: PluginInstanceId,
    },
    #[error(
        "HTTP route surface/path mismatch for {method} {path}: surface {surface}, owner {owner}"
    )]
    HttpSurfacePath {
        surface: String,
        method: String,
        path: String,
        owner: PluginInstanceId,
    },
    #[error(
        "HTTP route {method} {path} from {registration_source} owner {owner} overlaps reserved Host route {reserved_method} {reserved_path}"
    )]
    ReservedHttpRoute {
        registration_source: RegistrationSource,
        method: String,
        path: String,
        owner: PluginInstanceId,
        reserved_method: String,
        reserved_path: String,
    },
    #[error(
        "Admin page conflict for {path}: existing owner {existing_owner}, incoming owner {incoming_owner}"
    )]
    AdminPage {
        path: String,
        existing_owner: PluginInstanceId,
        incoming_owner: PluginInstanceId,
    },
    #[error(
        "CLI command conflict for {name}: existing owner {existing_owner}, incoming owner {incoming_owner}"
    )]
    CliCommand {
        name: String,
        existing_owner: PluginInstanceId,
        incoming_owner: PluginInstanceId,
    },
    #[error(
        "menu contribution conflict for {id}: existing owner {existing_owner}, incoming owner {incoming_owner}"
    )]
    MenuContribution {
        id: String,
        existing_owner: PluginInstanceId,
        incoming_owner: PluginInstanceId,
    },
    #[error(
        "Template root conflict for {plugin_id}: existing owner {existing_owner}, incoming owner {incoming_owner}"
    )]
    TemplateRoot {
        plugin_id: String,
        existing_owner: PluginInstanceId,
        incoming_owner: PluginInstanceId,
    },
    #[error(
        "Static root conflict for {plugin_id}: existing owner {existing_owner}, incoming owner {incoming_owner}"
    )]
    StaticRoot {
        plugin_id: String,
        existing_owner: PluginInstanceId,
        incoming_owner: PluginInstanceId,
    },
}

fn build_committed_snapshot(
    current: &CapabilitySnapshot,
    staged: StagedRegistrar,
    mode: CommitMode,
) -> Result<CapabilitySnapshot, RegistrationConflict> {
    validate_http_routes(current, &staged)?;
    validate_admin_pages(current, &staged)?;
    validate_cli_commands(current, &staged)?;
    validate_menu_contributions(current, &staged)?;
    validate_template_roots(current, &staged)?;
    validate_static_roots(current, &staged)?;

    let mut next = current.clone();
    match mode {
        CommitMode::Merge => {
            let http_keys = staged
                .http_routes
                .iter()
                .map(|spec| (spec.method.clone(), spec.path.clone()))
                .collect::<BTreeSet<_>>();
            let admin_keys = staged
                .admin_pages
                .iter()
                .map(|spec| spec.path.clone())
                .collect::<BTreeSet<_>>();
            let cli_keys = staged
                .cli_commands
                .iter()
                .map(|spec| spec.name.clone())
                .collect::<BTreeSet<_>>();
            let menu_keys = staged
                .menu_contributions
                .iter()
                .map(|spec| spec.id.clone())
                .collect::<BTreeSet<_>>();
            let template_keys = staged
                .template_roots
                .iter()
                .map(|spec| spec.plugin_id.clone())
                .collect::<BTreeSet<_>>();
            let static_keys = staged
                .static_roots
                .iter()
                .map(|spec| spec.plugin_id.clone())
                .collect::<BTreeSet<_>>();
            let event_keys = staged
                .event_subscriptions
                .iter()
                .map(|spec| spec.subscription_id)
                .collect::<BTreeSet<_>>();

            next.http_routes.retain(|registration| {
                !(registration.owner == staged.owner
                    && http_keys.contains(&(
                        registration.value.method.clone(),
                        registration.value.path.clone(),
                    )))
            });
            next.admin_pages.retain(|registration| {
                !(registration.owner == staged.owner
                    && admin_keys.contains(&registration.value.path))
            });
            next.cli_commands.retain(|registration| {
                !(registration.owner == staged.owner && cli_keys.contains(&registration.value.name))
            });
            next.menu_contributions.retain(|registration| {
                !(registration.owner == staged.owner && menu_keys.contains(&registration.value.id))
            });
            next.template_roots.retain(|registration| {
                !(registration.owner == staged.owner
                    && template_keys.contains(&registration.value.plugin_id))
            });
            next.static_roots.retain(|registration| {
                !(registration.owner == staged.owner
                    && static_keys.contains(&registration.value.plugin_id))
            });
            next.event_subscriptions.retain(|registration| {
                !(registration.owner == staged.owner
                    && event_keys.contains(&registration.value.subscription_id))
            });
        }
        CommitMode::ReplaceOwner => {
            next.http_routes
                .retain(|registration| registration.owner != staged.owner);
            next.admin_pages
                .retain(|registration| registration.owner != staged.owner);
            next.cli_commands
                .retain(|registration| registration.owner != staged.owner);
            next.menu_contributions
                .retain(|registration| registration.owner != staged.owner);
            next.template_roots
                .retain(|registration| registration.owner != staged.owner);
            next.static_roots
                .retain(|registration| registration.owner != staged.owner);
            next.event_subscriptions
                .retain(|registration| registration.owner != staged.owner);
        }
    }

    next.http_routes.extend(
        staged
            .http_routes
            .into_iter()
            .map(|value| OwnedRegistration {
                id: RegistrationId::next(),
                owner: staged.owner.clone(),
                source: staged.source,
                value,
            }),
    );
    next.admin_pages.extend(
        staged
            .admin_pages
            .into_iter()
            .map(|value| OwnedRegistration {
                id: RegistrationId::next(),
                owner: staged.owner.clone(),
                source: staged.source,
                value,
            }),
    );
    next.cli_commands.extend(
        staged
            .cli_commands
            .into_iter()
            .map(|value| OwnedRegistration {
                id: RegistrationId::next(),
                owner: staged.owner.clone(),
                source: staged.source,
                value,
            }),
    );
    next.menu_contributions
        .extend(
            staged
                .menu_contributions
                .into_iter()
                .map(|value| OwnedRegistration {
                    id: RegistrationId::next(),
                    owner: staged.owner.clone(),
                    source: staged.source,
                    value,
                }),
        );
    next.template_roots.extend(
        staged
            .template_roots
            .into_iter()
            .map(|value| OwnedRegistration {
                id: RegistrationId::next(),
                owner: staged.owner.clone(),
                source: staged.source,
                value,
            }),
    );
    next.static_roots.extend(
        staged
            .static_roots
            .into_iter()
            .map(|value| OwnedRegistration {
                id: RegistrationId::next(),
                owner: staged.owner.clone(),
                source: staged.source,
                value,
            }),
    );
    next.event_subscriptions
        .extend(
            staged
                .event_subscriptions
                .into_iter()
                .map(|value| OwnedRegistration {
                    id: RegistrationId::next(),
                    owner: staged.owner.clone(),
                    source: staged.source,
                    value,
                }),
        );
    sort_snapshot(&mut next);
    Ok(next)
}

fn validate_http_routes(
    current: &CapabilitySnapshot,
    staged: &StagedRegistrar,
) -> Result<(), RegistrationConflict> {
    let mut owners = current
        .http_routes
        .iter()
        .map(|entry| {
            (
                (entry.value.method.clone(), entry.value.path.clone()),
                entry.owner.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut staged_keys = BTreeSet::new();

    for spec in &staged.http_routes {
        if spec.surface != HttpSurface::from_path(&spec.path) {
            return Err(RegistrationConflict::HttpSurfacePath {
                surface: spec.surface.as_str().to_string(),
                method: spec.method.clone(),
                path: spec.path.clone(),
                owner: staged.owner.clone(),
            });
        }
        if staged.source != RegistrationSource::Builtin {
            if let Some(reserved) =
                conflicting_reserved_http_route(spec.surface, &spec.method, &spec.path)
            {
                return Err(RegistrationConflict::ReservedHttpRoute {
                    registration_source: staged.source,
                    method: spec.method.clone(),
                    path: spec.path.clone(),
                    owner: staged.owner.clone(),
                    reserved_method: reserved.method.to_string(),
                    reserved_path: reserved.path.to_string(),
                });
            }
        }
        let key = (spec.method.clone(), spec.path.clone());
        if !staged_keys.insert(key.clone()) {
            return Err(RegistrationConflict::HttpRoute {
                method: spec.method.clone(),
                path: spec.path.clone(),
                existing_owner: staged.owner.clone(),
                incoming_owner: staged.owner.clone(),
            });
        }
        if let Some(existing_owner) = owners.get(&key) {
            if existing_owner != &staged.owner {
                return Err(RegistrationConflict::HttpRoute {
                    method: spec.method.clone(),
                    path: spec.path.clone(),
                    existing_owner: existing_owner.clone(),
                    incoming_owner: staged.owner.clone(),
                });
            }
        }
        owners.insert(key, staged.owner.clone());
    }
    Ok(())
}

fn validate_admin_pages(
    current: &CapabilitySnapshot,
    staged: &StagedRegistrar,
) -> Result<(), RegistrationConflict> {
    let mut owners = current
        .admin_pages
        .iter()
        .map(|entry| (entry.value.path.clone(), entry.owner.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut staged_paths = BTreeSet::new();

    for spec in &staged.admin_pages {
        if staged.source != RegistrationSource::Builtin {
            if let Some(reserved) =
                conflicting_reserved_http_route(HttpSurface::Admin, "GET", &spec.path)
            {
                return Err(RegistrationConflict::ReservedHttpRoute {
                    registration_source: staged.source,
                    method: "GET".to_string(),
                    path: spec.path.clone(),
                    owner: staged.owner.clone(),
                    reserved_method: reserved.method.to_string(),
                    reserved_path: reserved.path.to_string(),
                });
            }
        }
        if !staged_paths.insert(spec.path.clone()) {
            return Err(RegistrationConflict::AdminPage {
                path: spec.path.clone(),
                existing_owner: staged.owner.clone(),
                incoming_owner: staged.owner.clone(),
            });
        }
        if let Some(existing_owner) = owners.get(&spec.path) {
            if existing_owner != &staged.owner {
                return Err(RegistrationConflict::AdminPage {
                    path: spec.path.clone(),
                    existing_owner: existing_owner.clone(),
                    incoming_owner: staged.owner.clone(),
                });
            }
        }
        owners.insert(spec.path.clone(), staged.owner.clone());
    }
    Ok(())
}

fn conflicting_reserved_http_route(
    surface: HttpSurface,
    method: &str,
    path: &str,
) -> Option<&'static ReservedHttpRoute> {
    HOST_RESERVED_HTTP_ROUTES
        .iter()
        .filter(|reserved| reserved.surface == surface && reserved.method == method)
        .filter(|reserved| http_path_patterns_overlap(path, reserved.path))
        .min_by(|left, right| left.path.cmp(right.path))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HttpPathSegment<'a> {
    Literal(&'a str),
    Parameter,
    CatchAll,
}

fn http_path_patterns_overlap(left: &str, right: &str) -> bool {
    let left = http_path_segments(left);
    let right = http_path_segments(right);
    path_segments_overlap(&left, &right)
}

fn http_path_match_score(pattern: &str, path: &str) -> Option<u32> {
    let pattern_segments = http_path_segments(pattern);
    let path_segments = path
        .strip_prefix('/')
        .unwrap_or(path)
        .split('/')
        .collect::<Vec<_>>();
    let mut pattern_index = 0;
    let mut path_index = 0;
    let mut score = 0;

    while pattern_index < pattern_segments.len() {
        match pattern_segments[pattern_index] {
            HttpPathSegment::CatchAll => return Some(score),
            HttpPathSegment::Literal(expected) => {
                if path_segments.get(path_index).copied() != Some(expected) {
                    return None;
                }
                score += 3;
                path_index += 1;
            }
            HttpPathSegment::Parameter => {
                if path_segments
                    .get(path_index)
                    .is_none_or(|value| value.is_empty())
                {
                    return None;
                }
                score += 2;
                path_index += 1;
            }
        }
        pattern_index += 1;
    }

    (path_index == path_segments.len()).then_some(score + 1)
}

fn http_path_segments(path: &str) -> Vec<HttpPathSegment<'_>> {
    path.strip_prefix('/')
        .unwrap_or(path)
        .split('/')
        .map(|segment| {
            if segment == "*" || (segment.starts_with("{*") && segment.ends_with('}')) {
                HttpPathSegment::CatchAll
            } else if segment.starts_with('{') && segment.ends_with('}') {
                HttpPathSegment::Parameter
            } else {
                HttpPathSegment::Literal(segment)
            }
        })
        .collect()
}

fn path_segments_overlap(left: &[HttpPathSegment<'_>], right: &[HttpPathSegment<'_>]) -> bool {
    match (left.first(), right.first()) {
        (None, None) => true,
        (Some(HttpPathSegment::CatchAll), _) | (_, Some(HttpPathSegment::CatchAll)) => true,
        (None, _) | (_, None) => false,
        (
            Some(HttpPathSegment::Literal(left_literal)),
            Some(HttpPathSegment::Literal(right_literal)),
        ) => left_literal == right_literal && path_segments_overlap(&left[1..], &right[1..]),
        (Some(HttpPathSegment::Literal(value)), Some(HttpPathSegment::Parameter))
        | (Some(HttpPathSegment::Parameter), Some(HttpPathSegment::Literal(value))) => {
            !value.is_empty() && path_segments_overlap(&left[1..], &right[1..])
        }
        (Some(HttpPathSegment::Parameter), Some(HttpPathSegment::Parameter)) => {
            path_segments_overlap(&left[1..], &right[1..])
        }
    }
}

fn validate_cli_commands(
    current: &CapabilitySnapshot,
    staged: &StagedRegistrar,
) -> Result<(), RegistrationConflict> {
    let mut owners = current
        .cli_commands
        .iter()
        .map(|entry| (entry.value.name.clone(), entry.owner.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut staged_names = BTreeSet::new();

    for spec in &staged.cli_commands {
        if let Some(existing_owner) = owners.get(&spec.name) {
            if existing_owner != &staged.owner {
                return Err(RegistrationConflict::CliCommand {
                    name: spec.name.clone(),
                    existing_owner: existing_owner.clone(),
                    incoming_owner: staged.owner.clone(),
                });
            }
        }
        if !staged_names.insert(spec.name.clone()) {
            return Err(RegistrationConflict::CliCommand {
                name: spec.name.clone(),
                existing_owner: staged.owner.clone(),
                incoming_owner: staged.owner.clone(),
            });
        }
        owners.insert(spec.name.clone(), staged.owner.clone());
    }
    Ok(())
}

fn validate_template_roots(
    current: &CapabilitySnapshot,
    staged: &StagedRegistrar,
) -> Result<(), RegistrationConflict> {
    let owners = current
        .template_roots
        .iter()
        .map(|entry| (entry.value.plugin_id.clone(), entry.owner.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut staged_ids = BTreeSet::new();

    for spec in &staged.template_roots {
        if !staged_ids.insert(spec.plugin_id.clone()) {
            return Err(RegistrationConflict::TemplateRoot {
                plugin_id: spec.plugin_id.clone(),
                existing_owner: staged.owner.clone(),
                incoming_owner: staged.owner.clone(),
            });
        }
        if let Some(existing_owner) = owners.get(&spec.plugin_id) {
            if existing_owner != &staged.owner {
                return Err(RegistrationConflict::TemplateRoot {
                    plugin_id: spec.plugin_id.clone(),
                    existing_owner: existing_owner.clone(),
                    incoming_owner: staged.owner.clone(),
                });
            }
        }
    }
    Ok(())
}

fn validate_menu_contributions(
    current: &CapabilitySnapshot,
    staged: &StagedRegistrar,
) -> Result<(), RegistrationConflict> {
    let mut owners = current
        .menu_contributions
        .iter()
        .map(|entry| (entry.value.id.clone(), entry.owner.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut staged_ids = BTreeSet::new();

    for spec in &staged.menu_contributions {
        if !staged_ids.insert(spec.id.clone()) {
            return Err(RegistrationConflict::MenuContribution {
                id: spec.id.clone(),
                existing_owner: staged.owner.clone(),
                incoming_owner: staged.owner.clone(),
            });
        }
        if let Some(existing_owner) = owners.get(&spec.id) {
            if existing_owner != &staged.owner {
                return Err(RegistrationConflict::MenuContribution {
                    id: spec.id.clone(),
                    existing_owner: existing_owner.clone(),
                    incoming_owner: staged.owner.clone(),
                });
            }
        }
        owners.insert(spec.id.clone(), staged.owner.clone());
    }
    Ok(())
}

fn validate_static_roots(
    current: &CapabilitySnapshot,
    staged: &StagedRegistrar,
) -> Result<(), RegistrationConflict> {
    let owners = current
        .static_roots
        .iter()
        .map(|entry| (entry.value.plugin_id.clone(), entry.owner.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut staged_ids = BTreeSet::new();

    for spec in &staged.static_roots {
        if !staged_ids.insert(spec.plugin_id.clone()) {
            return Err(RegistrationConflict::StaticRoot {
                plugin_id: spec.plugin_id.clone(),
                existing_owner: staged.owner.clone(),
                incoming_owner: staged.owner.clone(),
            });
        }
        if let Some(existing_owner) = owners.get(&spec.plugin_id) {
            if existing_owner != &staged.owner {
                return Err(RegistrationConflict::StaticRoot {
                    plugin_id: spec.plugin_id.clone(),
                    existing_owner: existing_owner.clone(),
                    incoming_owner: staged.owner.clone(),
                });
            }
        }
    }
    Ok(())
}

fn sort_snapshot(snapshot: &mut CapabilitySnapshot) {
    snapshot.http_routes.sort_by(|left, right| {
        (&left.value.surface, &left.value.method, &left.value.path).cmp(&(
            &right.value.surface,
            &right.value.method,
            &right.value.path,
        ))
    });
    snapshot
        .admin_pages
        .sort_by(|left, right| left.value.path.cmp(&right.value.path));
    snapshot
        .cli_commands
        .sort_by(|left, right| left.value.name.cmp(&right.value.name));
    snapshot.menu_contributions.sort_by(|left, right| {
        (&left.value.position, &left.value.id).cmp(&(&right.value.position, &right.value.id))
    });
    snapshot
        .template_roots
        .sort_by(|left, right| left.value.plugin_id.cmp(&right.value.plugin_id));
    snapshot
        .static_roots
        .sort_by(|left, right| left.value.plugin_id.cmp(&right.value.plugin_id));
    snapshot.event_subscriptions.sort_by(|left, right| {
        (&left.value.event, left.value.subscription_id)
            .cmp(&(&right.value.event, right.value.subscription_id))
    });
}
