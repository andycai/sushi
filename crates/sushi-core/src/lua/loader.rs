use crate::auth::policy_repository::PolicyRepository;
use crate::context::SushiContext;
use crate::fs::FileBrowserFsService;
use crate::lua::bindings::{inject_sushi_api, inject_sushi_fs};
use crate::lua::module_loader::install_plugin_require;
use crate::lua::vm::create_sandboxed_vm;
use crate::plugin::manager::PageResolvedAssets;
use crate::plugin::{
    Permissions, Plugin, PluginError, PluginFileBrowserConfig, PluginKind, PluginManifest,
};
use crate::storage::Storage;
use async_trait::async_trait;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// A Lua-based plugin loaded from the filesystem.
pub struct LuaPlugin {
    manifest: PluginManifest,
    kind: PluginKind,
    effective_permissions: Permissions,
    lua: Option<mlua::Lua>,
    plugin_dir: PathBuf,
    plugin_path_id: String,
}

impl LuaPlugin {
    /// Scan a directory for plugins in tiered official/third_party layout.
    pub async fn scan_dir(dir: &Path) -> Result<Vec<Self>, PluginError> {
        let mut plugins = Vec::new();
        let mut legacy_dirs = Vec::new();
        let mut entries = tokio::fs::read_dir(dir)
            .await
            .map_err(PluginError::IoError)?;

        while let Some(entry) = entries.next_entry().await.map_err(PluginError::IoError)? {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let category_name = entry.file_name().to_string_lossy().to_string();
            if category_name != "official" && category_name != "third_party" {
                if path.join("plugin.toml").is_file() {
                    legacy_dirs.push(path.display().to_string());
                }
                continue;
            }

            let expected_kind = if category_name == "official" {
                PluginKind::Official
            } else {
                PluginKind::ThirdParty
            };

            let mut plugin_entries = tokio::fs::read_dir(&path)
                .await
                .map_err(PluginError::IoError)?;
            while let Some(plugin_entry) = plugin_entries
                .next_entry()
                .await
                .map_err(PluginError::IoError)?
            {
                let plugin_path = plugin_entry.path();
                if !plugin_path.is_dir() {
                    continue;
                }

                let manifest_path = plugin_path.join("plugin.toml");
                if !manifest_path.exists() {
                    continue;
                }

                let manifest_content =
                    tokio::fs::read_to_string(&manifest_path)
                        .await
                        .map_err(|e| {
                            PluginError::ManifestError(format!(
                                "read {}: {e}",
                                manifest_path.display()
                            ))
                        })?;
                let (manifest, manifest_kind) = PluginManifest::parse_with_kind(&manifest_content)
                    .map_err(|e| {
                        PluginError::ManifestError(format!(
                            "parse {}: {e}",
                            manifest_path.display()
                        ))
                    })?;

                validate_file_browser_config(&manifest).map_err(|message| {
                    PluginError::ManifestError(format!(
                        "plugin '{}' file_browser config invalid: {message}",
                        manifest.plugin.name
                    ))
                })?;

                if manifest_kind != expected_kind {
                    return Err(PluginError::ManifestError(format!(
                        "plugin '{}' kind '{}' does not match '{}' directory",
                        manifest.plugin.name,
                        manifest_kind.tier_name(),
                        expected_kind.tier_name()
                    )));
                }

                let effective_permissions =
                    manifest_kind.effective_permissions(&manifest.permissions);
                let plugin_dir_name = plugin_entry.file_name().to_string_lossy().to_string();
                let plugin_path_id = format!("{}/{}", expected_kind.tier_name(), plugin_dir_name);

                let lua = create_sandboxed_vm().map_err(|e| {
                    PluginError::LuaError(format!("create VM for {}: {e}", manifest.plugin.name))
                })?;

                plugins.push(Self {
                    manifest,
                    kind: manifest_kind,
                    effective_permissions,
                    lua: Some(lua),
                    plugin_dir: plugin_path,
                    plugin_path_id,
                });
            }
        }

        if !legacy_dirs.is_empty() {
            return Err(PluginError::ManifestError(format!(
                "legacy flat plugin directories are not supported; move these into plugins/official or plugins/third_party: {}",
                legacy_dirs.join(", ")
            )));
        }

        Ok(plugins)
    }

    pub fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    pub fn kind(&self) -> PluginKind {
        self.kind
    }

    pub fn path_id(&self) -> &str {
        &self.plugin_path_id
    }

    pub fn effective_permissions(&self) -> &Permissions {
        &self.effective_permissions
    }

    pub fn web_templates_dir(&self) -> PathBuf {
        self.plugin_dir.join("web").join("templates")
    }

    pub fn web_static_dir(&self) -> PathBuf {
        self.plugin_dir.join("web").join("static")
    }

    /// Take the Lua VM out of the plugin after init.
    /// This transfers ownership to the caller (typically PluginManager).
    pub fn into_vm(self) -> Option<mlua::Lua> {
        self.lua
    }
}

fn validate_file_browser_config(manifest: &PluginManifest) -> Result<(), String> {
    let Some(config) = &manifest.file_browser else {
        return Ok(());
    };

    validate_route_prefix(config)?;
    validate_text_extensions(config)?;
    validate_roots(config)?;
    Ok(())
}

fn validate_route_prefix(config: &PluginFileBrowserConfig) -> Result<(), String> {
    let route_prefix = config.route_prefix.as_str();
    if route_prefix.is_empty() {
        return Err("route_prefix must be non-empty".to_string());
    }
    if route_prefix.trim() != route_prefix {
        return Err("route_prefix cannot contain leading/trailing whitespace".to_string());
    }
    if !route_prefix.starts_with('/') {
        return Err(format!("route_prefix '{route_prefix}' must start with '/'"));
    }
    Ok(())
}

fn validate_text_extensions(config: &PluginFileBrowserConfig) -> Result<(), String> {
    for ext in &config.text_extensions {
        let trimmed = ext.trim();
        if trimmed.is_empty() {
            return Err("text_extensions cannot contain empty values".to_string());
        }
        let normalized = trimmed.trim_start_matches('.');
        if normalized.is_empty()
            || !normalized
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(format!(
                "text_extensions entry '{ext}' is invalid; expected extension token"
            ));
        }
    }
    Ok(())
}

fn validate_roots(config: &PluginFileBrowserConfig) -> Result<(), String> {
    let mut seen_ids = HashSet::new();

    for root in &config.roots {
        let id = root.id.as_str();
        if id.is_empty() {
            return Err("root id must be non-empty".to_string());
        }
        if !id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
        {
            return Err(format!(
                "root id '{id}' is invalid; expected pattern [a-z0-9-_]+"
            ));
        }
        if !seen_ids.insert(id.to_string()) {
            return Err(format!("duplicate root id '{id}'"));
        }

        if root.path.trim() != root.path {
            return Err(format!(
                "root path '{}' for id '{id}' cannot contain leading/trailing whitespace",
                root.path
            ));
        }
        let path = Path::new(root.path.as_str());
        if path.as_os_str().is_empty() {
            return Err(format!("root path for id '{id}' must be non-empty"));
        }
        for component in path.components() {
            match component {
                std::path::Component::ParentDir => {
                    return Err(format!(
                        "root path '{}' for id '{id}' cannot contain '..'",
                        root.path
                    ));
                }
                std::path::Component::Prefix(_) => {
                    return Err(format!(
                        "root path '{}' for id '{id}' has invalid prefix",
                        root.path
                    ));
                }
                _ => {}
            }
        }
    }

    Ok(())
}

fn normalize_static_url_prefix(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "/static".to_string();
    }

    let mut prefix = if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    };

    if prefix.len() > 1 {
        prefix = prefix.trim_end_matches('/').to_string();
    }

    if prefix == "/" {
        return "/static".to_string();
    }

    prefix
}

fn parse_optional_string_array(
    entry: &mlua::Table,
    field: &str,
) -> Result<Vec<String>, PluginError> {
    match entry
        .get::<mlua::Value>(field)
        .map_err(|e| PluginError::InitFailed(format!("invalid page assets.{field}: {e}")))?
    {
        mlua::Value::Nil => Ok(Vec::new()),
        mlua::Value::Table(values) => {
            let len = values.raw_len();
            let mut entries = 0usize;
            for pair in values.pairs::<mlua::Value, mlua::Value>() {
                let (key, _) = pair.map_err(|e| {
                    PluginError::InitFailed(format!("invalid page assets.{field} keys: {e}"))
                })?;
                entries += 1;
                match key {
                    mlua::Value::Integer(index) if index >= 1 && (index as usize) <= len => {}
                    _ => {
                        return Err(PluginError::InitFailed(format!(
                            "page assets.{field} must be an array of strings"
                        )))
                    }
                }
            }
            if entries != len {
                return Err(PluginError::InitFailed(format!(
                    "page assets.{field} must be an array of strings"
                )));
            }

            let mut out = Vec::with_capacity(len);
            for index in 1..=len {
                let value = values.get::<mlua::Value>(index).map_err(|e| {
                    PluginError::InitFailed(format!(
                        "invalid page assets.{field}[{index}] value: {e}"
                    ))
                })?;
                let item = match value {
                    mlua::Value::String(item) => item
                        .to_str()
                        .map_err(|e| {
                            PluginError::InitFailed(format!(
                                "invalid utf-8 in page assets.{field}[{index}]: {e}"
                            ))
                        })?
                        .to_string(),
                    _ => {
                        return Err(PluginError::InitFailed(format!(
                            "page assets.{field} entries must be strings"
                        )))
                    }
                };
                out.push(item);
            }
            Ok(out)
        }
        _ => Err(PluginError::InitFailed(format!(
            "page assets.{field} must be an array of strings"
        ))),
    }
}

fn parse_page_assets_entry(
    entry: &mlua::Table,
) -> Result<(Vec<String>, Vec<String>, Vec<String>), PluginError> {
    let assets_value = entry
        .get::<mlua::Value>("assets")
        .map_err(|e| PluginError::InitFailed(format!("invalid page assets field: {e}")))?;
    let mlua::Value::Table(assets) = assets_value else {
        return Ok((Vec::new(), Vec::new(), Vec::new()));
    };

    let bundles = parse_optional_string_array(&assets, "bundles")?;
    let js = parse_optional_string_array(&assets, "js")?;
    let css = parse_optional_string_array(&assets, "css")?;

    Ok((bundles, js, css))
}

fn validate_resolvable_relative_path(path: &str, field: &str) -> Result<(), PluginError> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(PluginError::InitFailed(format!(
            "invalid assets.{field} path: empty value"
        )));
    }
    if Path::new(trimmed).is_absolute() {
        return Err(PluginError::InitFailed(format!(
            "invalid assets.{field} path '{trimmed}': absolute paths are not allowed"
        )));
    }
    if trimmed.contains("..") {
        return Err(PluginError::InitFailed(format!(
            "invalid assets.{field} path '{trimmed}': parent directory segments are not allowed"
        )));
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") || trimmed.starts_with("//") {
        return Err(PluginError::InitFailed(format!(
            "invalid assets.{field} path '{trimmed}': URL values are not allowed"
        )));
    }
    Ok(())
}

fn push_resolved_assets(
    plugin_path_id: &str,
    static_url_prefix: &str,
    static_root: &Path,
    source_paths: &[String],
    field: &str,
    target: &mut Vec<String>,
    seen: &mut HashSet<String>,
) -> Result<(), PluginError> {
    for path in source_paths {
        let normalized_path = path.trim().to_string();
        validate_resolvable_relative_path(&normalized_path, field)?;
        if !seen.insert(normalized_path.clone()) {
            continue;
        }

        let file_path = static_root.join(&normalized_path);
        if !file_path.is_file() {
            return Err(PluginError::InitFailed(format!(
                "missing plugin asset file '{}'",
                file_path.display()
            )));
        }

        target.push(format!(
            "{static_url_prefix}/plugins/{plugin_path_id}/{}",
            normalized_path
        ));
    }

    Ok(())
}

fn resolve_page_assets(
    plugin_path_id: &str,
    manifest: &PluginManifest,
    bundle_names: &[String],
    page_js: &[String],
    page_css: &[String],
    static_root: &Path,
    static_url_prefix: &str,
) -> Result<PageResolvedAssets, PluginError> {
    let mut resolved = PageResolvedAssets::default();
    let mut seen_js = HashSet::new();
    let mut seen_css = HashSet::new();

    for bundle_name in bundle_names {
        let bundle = manifest
            .admin
            .as_ref()
            .and_then(|admin| admin.assets.as_ref())
            .and_then(|assets| assets.bundles.get(bundle_name))
            .ok_or_else(|| {
                PluginError::InitFailed(format!("unknown page asset bundle: {bundle_name}"))
            })?;

        push_resolved_assets(
            plugin_path_id,
            static_url_prefix,
            static_root,
            &bundle.js,
            "js",
            &mut resolved.js,
            &mut seen_js,
        )?;
        push_resolved_assets(
            plugin_path_id,
            static_url_prefix,
            static_root,
            &bundle.css,
            "css",
            &mut resolved.css,
            &mut seen_css,
        )?;
    }

    push_resolved_assets(
        plugin_path_id,
        static_url_prefix,
        static_root,
        page_js,
        "js",
        &mut resolved.js,
        &mut seen_js,
    )?;
    push_resolved_assets(
        plugin_path_id,
        static_url_prefix,
        static_root,
        page_css,
        "css",
        &mut resolved.css,
        &mut seen_css,
    )?;

    Ok(resolved)
}

fn policy_matches_scope(policy_key: &str, scope: &str) -> bool {
    if let Some(prefix) = scope.strip_suffix('*') {
        policy_key.starts_with(prefix)
    } else {
        policy_key == scope
    }
}

fn validate_policy_scope(
    plugin_name: &str,
    target: &str,
    policy_key: &str,
    scopes: &[String],
) -> Result<(), PluginError> {
    if scopes
        .iter()
        .any(|scope| policy_matches_scope(policy_key, scope))
    {
        return Ok(());
    }

    let scope_list = if scopes.is_empty() {
        "<none>".to_string()
    } else {
        scopes.join(", ")
    };

    Err(PluginError::InitFailed(format!(
        "plugin '{plugin_name}' declared policy '{policy_key}' for {target}, but it is outside manifest policy scopes: [{scope_list}]"
    )))
}

fn parse_entry_policy(
    entry: &mlua::Table,
    entry_name: &str,
) -> Result<Option<String>, PluginError> {
    entry
        .get::<Option<String>>("policy")
        .map_err(|e| PluginError::InitFailed(format!("invalid {entry_name} policy value: {e}")))
}

fn parse_entry_public(entry: &mlua::Table, entry_name: &str) -> Result<bool, PluginError> {
    entry
        .get::<Option<bool>>("public")
        .map_err(|e| PluginError::InitFailed(format!("invalid {entry_name} public flag: {e}")))
        .map(|value| value.unwrap_or(false))
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

        let file_browser_root_dir = {
            let cfg = ctx.config.get().await;
            cfg.file_browser.root_dir.clone()
        };

        let file_browser_fs = self
            .manifest
            .file_browser
            .as_ref()
            .map(|manifest| {
                FileBrowserFsService::from_manifest_with_root_base(
                    manifest,
                    Path::new(&file_browser_root_dir),
                )
            })
            .transpose()
            .map_err(|e| {
                PluginError::InitFailed(format!(
                    "{}: failed to build file_browser fs service: {e}",
                    self.manifest.plugin.name
                ))
            })?;

        // Inject sushi.* API into the Lua VM
        inject_sushi_api(lua, ctx, &self.effective_permissions)
            .await
            .map_err(|e| PluginError::LuaError(format!("inject API: {e}")))?;
        if let Some(service) = file_browser_fs {
            inject_sushi_fs(lua, Arc::new(service))
                .map_err(|e| PluginError::LuaError(format!("inject sushi.fs API: {e}")))?;
        }

        install_plugin_require(lua, &self.plugin_dir)
            .map_err(|e| PluginError::LuaError(format!("install plugin module loader: {e}")))?;

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
        let allowed_policy_scopes = &self.manifest.policies.scopes;
        let policy_repo = PolicyRepository::new({
            let storage: Arc<dyn Storage> = ctx.db.clone();
            storage
        });

        // Read pending routes, register with PluginManager
        if let Ok(pending) = sushi.get::<mlua::Table>("__pending_routes") {
            let len = pending.raw_len();
            for i in 1..=len {
                if let Ok(entry) = pending.get::<mlua::Table>(i) {
                    let method: String = entry.get("method").unwrap_or_default();
                    let path: String = entry.get("path").unwrap_or_default();
                    let handler_key: String = entry.get("handler_key").unwrap_or_default();
                    let policy_key = parse_entry_policy(&entry, "route entry")?;
                    let is_public = parse_entry_public(&entry, "route entry")?;
                    if is_public && policy_key.is_some() {
                        return Err(PluginError::InitFailed(format!(
                            "route {method} {path} cannot declare both policy and public=true"
                        )));
                    }
                    if let Some(policy_key_value) = policy_key.as_deref() {
                        validate_policy_scope(
                            plugin_name,
                            &format!("route {method} {path}"),
                            policy_key_value,
                            allowed_policy_scopes,
                        )?;
                        policy_repo
                            .upsert_plugin_http_binding(
                                "api",
                                &method,
                                &path,
                                policy_key_value,
                                plugin_name,
                            )
                            .await
                            .map_err(|err| {
                                PluginError::InitFailed(format!(
                                    "failed to persist policy binding for route {method} {path}: {err}"
                                ))
                            })?;
                    } else {
                        policy_repo
                            .delete_plugin_http_binding("api", &method, &path, plugin_name)
                            .await
                            .map_err(|err| {
                                PluginError::InitFailed(format!(
                                    "failed to clear stale policy binding for route {method} {path}: {err}"
                                ))
                            })?;
                    }
                    ctx.plugins
                        .register_api_handler_with_policy_and_public(
                            &method,
                            &path,
                            plugin_name,
                            &handler_key,
                            policy_key.as_deref(),
                            is_public,
                        )
                        .await;
                    tracing::debug!(
                        "plugin {} registered route {} {} (handler: {}, policy: {:?}, public: {})",
                        plugin_name,
                        method,
                        path,
                        handler_key,
                        policy_key,
                        is_public
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
                    let policy_key = parse_entry_policy(&entry, "command entry")?;
                    if let Some(policy_key_value) = policy_key.as_deref() {
                        validate_policy_scope(
                            plugin_name,
                            &format!("command {name}"),
                            policy_key_value,
                            allowed_policy_scopes,
                        )?;
                        policy_repo
                            .upsert_plugin_cli_binding(&name, policy_key_value, plugin_name)
                            .await
                            .map_err(|err| {
                                PluginError::InitFailed(format!(
                                    "failed to persist policy binding for command {name}: {err}"
                                ))
                            })?;
                    } else {
                        policy_repo
                            .delete_plugin_cli_binding(&name, plugin_name)
                            .await
                            .map_err(|err| {
                                PluginError::InitFailed(format!(
                                    "failed to clear stale policy binding for command {name}: {err}"
                                ))
                            })?;
                    }
                    ctx.plugins
                        .register_cli_handler_with_policy(
                            &name,
                            plugin_name,
                            &handler_key,
                            policy_key.as_deref(),
                        )
                        .await;
                    tracing::debug!(
                        "plugin {} registered command {} (handler: {}, policy: {:?})",
                        plugin_name,
                        name,
                        handler_key,
                        policy_key
                    );
                }
            }
        }

        // Read pending pages, register with PluginManager
        if let Ok(pending) = sushi.get::<mlua::Table>("__pending_pages") {
            let static_prefix = {
                let cfg = ctx.config.get().await;
                normalize_static_url_prefix(&cfg.web.static_url_prefix)
            };
            let plugin_static_root = self.web_static_dir();
            let len = pending.raw_len();
            for i in 1..=len {
                if let Ok(entry) = pending.get::<mlua::Table>(i) {
                    let path: String = entry.get("path").unwrap_or_default();
                    let title: String = entry.get("title").unwrap_or_default();
                    let handler_key: String = entry.get("handler_key").unwrap_or_default();
                    let policy_key = parse_entry_policy(&entry, "page entry")?;
                    if let Some(policy_key_value) = policy_key.as_deref() {
                        validate_policy_scope(
                            plugin_name,
                            &format!("page {path}"),
                            policy_key_value,
                            allowed_policy_scopes,
                        )?;
                        policy_repo
                            .upsert_plugin_http_binding(
                                "admin",
                                "GET",
                                &path,
                                policy_key_value,
                                plugin_name,
                            )
                            .await
                            .map_err(|err| {
                                PluginError::InitFailed(format!(
                                    "failed to persist policy binding for page {path}: {err}"
                                ))
                            })?;
                    } else {
                        policy_repo
                            .delete_plugin_http_binding("admin", "GET", &path, plugin_name)
                            .await
                            .map_err(|err| {
                                PluginError::InitFailed(format!(
                                    "failed to clear stale policy binding for page {path}: {err}"
                                ))
                            })?;
                    }
                    let (bundle_names, page_js, page_css) = parse_page_assets_entry(&entry)?;
                    let assets = resolve_page_assets(
                        &self.plugin_path_id,
                        &self.manifest,
                        &bundle_names,
                        &page_js,
                        &page_css,
                        &plugin_static_root,
                        &static_prefix,
                    )?;
                    ctx.plugins
                        .register_admin_handler_with_assets_and_policy(
                            &path,
                            plugin_name,
                            &title,
                            &handler_key,
                            assets,
                            policy_key.as_deref(),
                        )
                        .await;
                    tracing::debug!(
                        "plugin {} registered page {} ({}) (handler: {}, policy: {:?})",
                        plugin_name,
                        path,
                        title,
                        handler_key,
                        policy_key
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
    use crate::plugin::PluginManifest;
    use crate::storage::sqlite::SqliteStorage;
    use crate::storage::Storage;
    use crate::web::template_service::TemplateService;
    use serde_json::Value;
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

    fn resolve_page_assets_for_test(
        plugin_path_id: &str,
        manifest: &PluginManifest,
        bundle_names: &[String],
        page_js: &[String],
        page_css: &[String],
        static_root: &Path,
    ) -> Result<PageResolvedAssets, PluginError> {
        resolve_page_assets(
            plugin_path_id,
            manifest,
            bundle_names,
            page_js,
            page_css,
            static_root,
            "/static",
        )
    }

    fn create_plugin_dir(parent: &Path, category: &str, name: &str, kind: &str) -> PathBuf {
        let dir = parent.join(category).join(name);
        std::fs::create_dir_all(&dir).unwrap();

        let manifest_content = format!(
            r#"
[plugin]
name = "{name}"
version = "0.1.0"
kind = "{kind}"
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

    fn create_plugin_dir_with_manifest(
        parent: &Path,
        category: &str,
        name: &str,
        manifest_content: &str,
    ) -> PathBuf {
        let dir = parent.join(category).join(name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("plugin.toml"), manifest_content).unwrap();
        std::fs::write(dir.join("init.lua"), "sushi.log.info('hello')").unwrap();
        dir
    }

    #[test]
    fn kv_store_plugin_no_longer_embeds_html() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
        let plugin_path = repo_root.join("plugins/official/kv-store/init.lua");
        let plugin_source = std::fs::read_to_string(&plugin_path).unwrap();

        assert!(!plugin_source.contains("<!DOCTYPE html>"));
        assert!(!plugin_source.contains("<html"));
        assert!(!plugin_source.contains("<div class=\\\"ui-flash"));
        assert!(!plugin_source.contains("sushi.admin.page"));
        assert!(plugin_source.contains("require(\"bootstrap.register\")"));
        assert!(plugin_source.contains("function sushi.init()"));

        let template_path = repo_root.join("plugins/official/kv-store/web/templates/kv.html");
        assert!(template_path.exists());
        let template_source = std::fs::read_to_string(&template_path).unwrap();
        assert!(template_source.contains("{% extends \"base.html\" %}"));
        assert!(!template_source.contains("http://"));
        assert!(!template_source.contains("https://"));

        let flash_template_path =
            repo_root.join("plugins/official/kv-store/web/templates/partials/flash.html");
        assert!(flash_template_path.exists());
        let flash_template_source = std::fs::read_to_string(&flash_template_path).unwrap();
        assert!(flash_template_source.contains("class=\"ui-flash {{ tone }}\""));

        let static_path = repo_root.join("plugins/official/kv-store/web/static/kv.js");
        assert!(static_path.exists());
        let static_source = std::fs::read_to_string(&static_path).unwrap();
        assert!(static_source.contains("kvPage"));
        assert!(!static_source.contains("http://"));
        assert!(!static_source.contains("https://"));
    }

    #[test]
    fn kv_store_plugin_declares_admin_asset_bundles() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
        let plugin_path = repo_root.join("plugins/official/kv-store/plugin.toml");
        let source = std::fs::read_to_string(plugin_path).unwrap();

        assert!(source.contains("kind = \"official\""));
        assert!(source.contains("[admin.assets.bundles.workspace]"));
        assert!(source.contains("js = [\"kv.js\"]"));
    }

    #[test]
    fn kv_store_registration_uses_page_assets_option() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
        let plugin_path = repo_root.join("plugins/official/kv-store/lua/bootstrap/register.lua");
        let source = std::fs::read_to_string(plugin_path).unwrap();

        assert!(source.contains("assets = {"));
        assert!(source.contains("bundles = { \"workspace\" }"));
    }

    #[test]
    fn kv_store_plugin_is_split_into_module_files() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
        assert!(repo_root
            .join("plugins/official/kv-store/lua/utils/json.lua")
            .is_file());
        assert!(repo_root
            .join("plugins/official/kv-store/lua/utils/form.lua")
            .is_file());
        assert!(repo_root
            .join("plugins/official/kv-store/lua/utils/html.lua")
            .is_file());
        assert!(repo_root
            .join("plugins/official/kv-store/lua/infra/db.lua")
            .is_file());
        assert!(repo_root
            .join("plugins/official/kv-store/lua/domain/store.lua")
            .is_file());
        assert!(repo_root
            .join("plugins/official/kv-store/lua/interfaces/api.lua")
            .is_file());
        assert!(repo_root
            .join("plugins/official/kv-store/lua/interfaces/admin.lua")
            .is_file());
        assert!(repo_root
            .join("plugins/official/kv-store/lua/interfaces/cli.lua")
            .is_file());
        assert!(repo_root
            .join("plugins/official/kv-store/lua/bootstrap/register.lua")
            .is_file());
    }

    #[test]
    fn kv_store_plugin_bootstrap_registration_contract_is_stable() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
        let plugin_path = repo_root.join("plugins/official/kv-store/lua/bootstrap/register.lua");
        let source = std::fs::read_to_string(plugin_path).unwrap();

        assert!(source.contains("function M.register(deps)"));
        assert!(source.contains(
            "sushi.api.route(\"GET\", \"/api/kv\", deps.api.dispatch, { policy = \"api.kv.read\" })"
        ));
        assert!(source.contains(
            "sushi.api.route(\"POST\", \"/api/kv\", deps.api.dispatch, { policy = \"api.kv.write\" })"
        ));
        assert!(source.contains(
            "sushi.api.route(\"DELETE\", \"/api/kv/*\", deps.api.delete_dispatch, { policy = \"api.kv.delete\" })"
        ));
        assert!(source.contains(
            "sushi.api.route(\"POST\", \"/admin/partials/kv/upsert\", deps.admin.upsert_partial, { policy = \"admin.kv.write\" })"
        ));
        assert!(source
            .contains("sushi.web.page(\"/admin/kv\", \"plugins/official/kv-store/kv.html\", {"));
        assert!(source.contains("policy = \"admin.kv.read\""));
        assert!(source.contains("assets = { bundles = { \"workspace\" } }"));
        assert!(source.contains("title = \"KV Store\""));
        assert!(source.contains(
            "sushi.cli.command(\"kv-set\", \"Set a KV entry (key + value)\", deps.cli.kv_set, { policy = \"cli.kv.set\" })"
        ));
        assert!(source.contains("return M"));
    }

    #[test]
    fn file_browser_plugin_is_split_into_module_files() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
        assert!(repo_root
            .join("plugins/official/file-browser/plugin.toml")
            .is_file());
        assert!(repo_root
            .join("plugins/official/file-browser/init.lua")
            .is_file());
        assert!(repo_root
            .join("plugins/official/file-browser/lua/bootstrap/register.lua")
            .is_file());
        assert!(repo_root
            .join("plugins/official/file-browser/lua/interfaces/web.lua")
            .is_file());
        assert!(repo_root
            .join("plugins/official/file-browser/lua/domain/browser.lua")
            .is_file());
        assert!(repo_root
            .join("plugins/official/file-browser/lua/utils/form.lua")
            .is_file());
        assert!(repo_root
            .join("plugins/official/file-browser/lua/utils/path.lua")
            .is_file());
        assert!(repo_root
            .join("plugins/official/file-browser/web/templates/file_browser.html")
            .is_file());
        assert!(repo_root
            .join("plugins/official/file-browser/web/templates/fragments/list.html")
            .is_file());
        assert!(repo_root
            .join("plugins/official/file-browser/web/templates/fragments/editor.html")
            .is_file());
        assert!(repo_root
            .join("plugins/official/file-browser/web/templates/fragments/flash.html")
            .is_file());
        assert!(repo_root
            .join("plugins/official/file-browser/web/static/file_browser.js")
            .is_file());
    }

    #[test]
    fn file_browser_plugin_bootstrap_registration_is_public_only() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
        let plugin_path =
            repo_root.join("plugins/official/file-browser/lua/bootstrap/register.lua");
        let source = std::fs::read_to_string(plugin_path).unwrap();

        assert!(source.contains("function M.register(app)"));
        assert!(source
            .contains("sushi.api.route(\"GET\", \"/app/files\", app.page, { public = true })"));
        assert!(source.contains(
            "sushi.api.route(\"GET\", \"/app/files/list/*\", app.list_partial, { public = true })"
        ));
        assert!(source.contains(
            "sushi.api.route(\"GET\", \"/app/files/open/*\", app.open_partial, { public = true })"
        ));
        assert!(source.contains(
            "sushi.api.route(\"POST\", \"/app/files/save/*\", app.save_text, { public = true })"
        ));
        assert!(source.contains("sushi.api.route(\"POST\", \"/app/files/create-text\", app.create_text, { public = true })"));
        assert!(source.contains("sushi.api.route(\"POST\", \"/app/files/create-dir\", app.create_dir, { public = true })"));
        assert!(source.contains(
            "sushi.api.route(\"POST\", \"/app/files/rename\", app.rename_entry, { public = true })"
        ));
        assert!(source.contains(
            "sushi.api.route(\"POST\", \"/app/files/delete\", app.delete_entry, { public = true })"
        ));
        assert!(source.contains("sushi.api.route(\"POST\", \"/app/files/upload/*\", app.upload_file, { public = true })"));
        assert!(source.contains("sushi.api.route(\"GET\", \"/app/files/download/*\", app.download_file, { public = true })"));
        assert!(source.contains("return M"));
    }

    #[test]
    fn cms_plugin_files_exist_and_are_modular() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
        assert!(repo_root.join("plugins/official/cms/plugin.toml").is_file());
        assert!(repo_root
            .join("plugins/official/cms/web/templates/cms.html")
            .is_file());
        assert!(repo_root
            .join("plugins/official/cms/web/templates/fragments/rows.html")
            .is_file());
        assert!(repo_root
            .join("plugins/official/cms/web/static/cms.js")
            .is_file());
        assert!(repo_root
            .join("plugins/official/cms/lua/interfaces/api.lua")
            .is_file());
        assert!(repo_root
            .join("plugins/official/cms/lua/interfaces/admin.lua")
            .is_file());
        assert!(repo_root
            .join("plugins/official/cms/lua/interfaces/cli.lua")
            .is_file());
    }

    #[test]
    fn cms_plugin_registration_contract_is_stable() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
        let plugin_path = repo_root.join("plugins/official/cms/lua/bootstrap/register.lua");
        let source = std::fs::read_to_string(plugin_path).unwrap();

        assert!(source.contains("sushi.web.page(\"/admin/cms\""));
        assert!(source.contains("sushi.api.route(\"GET\", \"/admin/partials/cms/overview\""));
        assert!(source.contains("sushi.api.route(\"GET\", \"/admin/partials/cms/library/*\""));
        assert!(source.contains("sushi.api.route(\"GET\", \"/admin/partials/cms/editor/*\""));
        assert!(source.contains("sushi.api.route(\"POST\", \"/admin/partials/cms/editor/save\""));
        assert!(
            source.contains("sushi.api.route(\"POST\", \"/admin/partials/cms/status/transition\"")
        );
        assert!(source.contains("sushi.api.route(\"GET\", \"/admin/partials/cms/commands\""));
        assert!(source.contains("sushi.api.route(\"GET\", \"/app/posts\""));
        assert!(source.contains("sushi.cli.command(\"cms\""));
    }

    #[test]
    fn cms_plugin_declares_workspace_css_bundle() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
        let source =
            std::fs::read_to_string(repo_root.join("plugins/official/cms/plugin.toml")).unwrap();

        assert!(source.contains("[admin.assets.bundles.workspace]"));
        assert!(source.contains("js = [\"cms.js\"]"));
        assert!(source.contains("css = [\"cms.css\"]"));
    }

    #[test]
    fn cms_utils_contract_is_stable() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
        let slug =
            std::fs::read_to_string(root.join("plugins/official/cms/lua/utils/slug.lua")).unwrap();
        let validate =
            std::fs::read_to_string(root.join("plugins/official/cms/lua/utils/validate.lua"))
                .unwrap();
        let markdown =
            std::fs::read_to_string(root.join("plugins/official/cms/lua/utils/markdown.lua"))
                .unwrap();

        assert!(slug.contains("function M.normalize"));
        assert!(validate.contains("function M.validate_status"));
        assert!(markdown.contains("function M.to_html"));
    }

    #[tokio::test]
    async fn test_scan_dir_finds_plugins() {
        let tmp = TempDir::new().unwrap();
        create_plugin_dir(tmp.path(), "official", "my_plugin", "official");

        let plugins = LuaPlugin::scan_dir(tmp.path()).await.unwrap();
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].name(), "my_plugin");
        assert_eq!(plugins[0].version(), "0.1.0");
        assert_eq!(plugins[0].path_id(), "official/my_plugin");
        assert_eq!(plugins[0].kind(), PluginKind::Official);
    }

    #[tokio::test]
    async fn test_scan_dir_skips_dirs_without_manifest() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("official").join("no_manifest");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("init.lua"), "print('hello')").unwrap();

        let plugins = LuaPlugin::scan_dir(tmp.path()).await.unwrap();
        assert_eq!(plugins.len(), 0);
    }

    #[tokio::test]
    async fn test_scan_dir_tiered_discovery_success() {
        let tmp = TempDir::new().unwrap();
        create_plugin_dir(tmp.path(), "official", "kv_store", "official");
        create_plugin_dir(tmp.path(), "third_party", "notes", "third_party");

        let mut plugins = LuaPlugin::scan_dir(tmp.path()).await.unwrap();
        plugins.sort_by(|left, right| left.path_id().cmp(right.path_id()));

        assert_eq!(plugins.len(), 2);
        assert_eq!(plugins[0].path_id(), "official/kv_store");
        assert_eq!(plugins[0].kind(), PluginKind::Official);
        assert_eq!(
            plugins[0].effective_permissions().database,
            crate::plugin::DatabasePermission::Admin
        );
        assert_eq!(plugins[1].path_id(), "third_party/notes");
        assert_eq!(plugins[1].kind(), PluginKind::ThirdParty);
        assert_eq!(
            plugins[1].effective_permissions().database,
            crate::plugin::DatabasePermission::None
        );
    }

    #[tokio::test]
    async fn test_scan_dir_rejects_legacy_flat_plugin_directory() {
        let tmp = TempDir::new().unwrap();
        create_plugin_dir(tmp.path(), "official", "modern", "official");

        let legacy = tmp.path().join("legacy_flat");
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::write(
            legacy.join("plugin.toml"),
            r#"
[plugin]
name = "legacy_flat"
version = "0.1.0"
kind = "third_party"
"#,
        )
        .unwrap();

        let result = LuaPlugin::scan_dir(tmp.path()).await;
        assert!(result.is_err());
        let err = result.err().unwrap().to_string();
        assert!(err.contains("legacy flat plugin directories"));
        assert!(err.contains("legacy_flat"));
    }

    #[tokio::test]
    async fn test_scan_dir_rejects_invalid_file_browser_config() {
        let tmp = TempDir::new().unwrap();
        create_plugin_dir_with_manifest(
            tmp.path(),
            "official",
            "bad_browser",
            r#"
[plugin]
name = "bad_browser"
version = "0.1.0"
kind = "official"

[file_browser]
route_prefix = "admin/files"
"#,
        );

        let result = LuaPlugin::scan_dir(tmp.path()).await;
        assert!(result.is_err());
        let err = result.err().unwrap().to_string();
        assert!(err.contains("file_browser config invalid"));
        assert!(err.contains("route_prefix"));
    }

    #[tokio::test]
    async fn test_scan_dir_accepts_relative_file_browser_root_paths() {
        let tmp = TempDir::new().unwrap();
        create_plugin_dir_with_manifest(
            tmp.path(),
            "official",
            "relative_browser",
            r#"
[plugin]
name = "relative_browser"
version = "0.1.0"
kind = "official"

[file_browser]
route_prefix = "/app/files"

[[file_browser.roots]]
id = "docs"
path = "docs"
"#,
        );

        let plugins = LuaPlugin::scan_dir(tmp.path()).await.unwrap();
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].name(), "relative_browser");
    }

    #[tokio::test]
    async fn test_scan_dir_rejects_whitespace_file_browser_values() {
        let tmp = TempDir::new().unwrap();
        let root_dir = tmp.path().join("fb_root");
        std::fs::create_dir_all(&root_dir).unwrap();

        create_plugin_dir_with_manifest(
            tmp.path(),
            "official",
            "bad_whitespace_route",
            &format!(
                r#"
[plugin]
name = "bad_whitespace_route"
version = "0.1.0"
kind = "official"

[file_browser]
route_prefix = " /app/files"

[[file_browser.roots]]
id = "docs"
path = "{}"
"#,
                root_dir.display()
            ),
        );

        let route_result = LuaPlugin::scan_dir(tmp.path()).await;
        assert!(route_result.is_err());
        let route_err = route_result.err().unwrap().to_string();
        assert!(route_err.contains("file_browser config invalid"));
        assert!(route_err.contains("route_prefix"));

        let tmp = TempDir::new().unwrap();
        let root_dir = tmp.path().join("fb_root");
        std::fs::create_dir_all(&root_dir).unwrap();

        create_plugin_dir_with_manifest(
            tmp.path(),
            "official",
            "bad_whitespace_id",
            &format!(
                r#"
[plugin]
name = "bad_whitespace_id"
version = "0.1.0"
kind = "official"

[file_browser]
route_prefix = "/app/files"

[[file_browser.roots]]
id = "docs "
path = "{}"
"#,
                root_dir.display()
            ),
        );

        let id_result = LuaPlugin::scan_dir(tmp.path()).await;
        assert!(id_result.is_err());
        let id_err = id_result.err().unwrap().to_string();
        assert!(id_err.contains("file_browser config invalid"));
        assert!(id_err.contains("root id"));

        let tmp = TempDir::new().unwrap();
        let root_dir = tmp.path().join("fb_root");
        std::fs::create_dir_all(&root_dir).unwrap();

        create_plugin_dir_with_manifest(
            tmp.path(),
            "official",
            "bad_whitespace_path",
            &format!(
                r#"
[plugin]
name = "bad_whitespace_path"
version = "0.1.0"
kind = "official"

[file_browser]
route_prefix = "/app/files"

[[file_browser.roots]]
id = "docs"
path = " {}"
"#,
                root_dir.display()
            ),
        );

        let path_result = LuaPlugin::scan_dir(tmp.path()).await;
        assert!(path_result.is_err());
        let path_err = path_result.err().unwrap().to_string();
        assert!(path_err.contains("file_browser config invalid"));
        assert!(path_err.contains("root path"));
    }

    #[tokio::test]
    async fn test_scan_dir_rejects_kind_category_mismatch() {
        let tmp = TempDir::new().unwrap();
        let plugin_dir = tmp.path().join("official").join("mismatch");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(
            plugin_dir.join("plugin.toml"),
            r#"
[plugin]
name = "mismatch"
version = "0.1.0"
kind = "third_party"
entry = "init.lua"
"#,
        )
        .unwrap();
        std::fs::write(plugin_dir.join("init.lua"), "sushi.log.info('hi')").unwrap();

        let result = LuaPlugin::scan_dir(tmp.path()).await;
        assert!(result.is_err());
        let err = result.err().unwrap().to_string();
        assert!(err.contains("does not match"));
        assert!(err.contains("official"));
    }

    #[tokio::test]
    async fn test_lua_plugin_init_executes_entry_script() {
        let tmp = TempDir::new().unwrap();
        create_plugin_dir(tmp.path(), "third_party", "test_plugin", "third_party");

        let plugins = LuaPlugin::scan_dir(tmp.path()).await.unwrap();
        let ctx = test_context().await;

        // init() should succeed without error
        plugins[0].init(&ctx).await.unwrap();
    }

    #[tokio::test]
    async fn test_lua_plugin_init_calls_sushi_init() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("third_party").join("init_fn_plugin");
        std::fs::create_dir_all(&dir).unwrap();

        std::fs::write(
            dir.join("plugin.toml"),
            r#"
[plugin]
name = "init_fn_plugin"
version = "0.1.0"
kind = "third_party"
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
    async fn disabled_plugin_is_not_invoked_after_scan_registration() {
        let ctx = test_context().await;
        ctx.db
            .run_migrations(include_str!("../../../../migrations/001_init.sql"))
            .await
            .unwrap();
        ctx.db
            .run_migrations(include_str!("../../../../migrations/003_rbac.sql"))
            .await
            .unwrap();
        ctx.db
            .run_migrations(include_str!("../../../../migrations/008_plugin_governance_v1.sql"))
            .await
            .unwrap();

        let manifest = PluginManifest {
            plugin: crate::plugin::PluginMeta {
                name: "notes".to_string(),
                version: "0.1.0".to_string(),
                description: "notes".to_string(),
                entry: "init.lua".to_string(),
            },
            permissions: crate::plugin::Permissions::default(),
            policies: crate::plugin::PluginPoliciesConfig::default(),
            admin: None,
            file_browser: None,
        };

        ctx.plugins
            .register_plugin_manifest_with_permissions_and_identity(
                &manifest,
                &crate::plugin::Permissions::default(),
                "third_party/notes",
                crate::plugin::PluginKind::ThirdParty,
            )
            .await;

        ctx.plugins
            .set_plugin_enabled("notes", false, Some("admin"), Some("seed"))
            .await
            .unwrap();

        assert_eq!(
            ctx.plugins
                .list_plugins()
                .await
                .into_iter()
                .find(|p| p.name == "notes")
                .unwrap()
                .enabled,
            false
        );
    }

    #[tokio::test]
    async fn plugin_load_fails_when_declared_policy_outside_scope() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("third_party").join("policy_mismatch");
        std::fs::create_dir_all(&dir).unwrap();

        std::fs::write(
            dir.join("plugin.toml"),
            r#"
[plugin]
name = "policy_mismatch"
version = "0.1.0"
kind = "third_party"
entry = "init.lua"

[permissions]
routes = true

[policies]
scopes = ["api.plugin.*"]
"#,
        )
        .unwrap();

        std::fs::write(
            dir.join("init.lua"),
            r#"
sushi.init = function()
    sushi.api.route("GET", "/api/mismatch", function()
        return "ok"
    end, { policy = "admin.users.read" })
end
"#,
        )
        .unwrap();

        let plugins = LuaPlugin::scan_dir(tmp.path()).await.unwrap();
        let ctx = test_context().await;
        let err = plugins[0].init(&ctx).await.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("policy_mismatch"));
        assert!(msg.contains("admin.users.read"));
        assert!(msg.contains("api.plugin.*"));
    }

    #[tokio::test]
    async fn plugin_load_persists_policy_metadata_for_registrations() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("third_party").join("policy_capture");
        std::fs::create_dir_all(&dir).unwrap();

        std::fs::write(
            dir.join("plugin.toml"),
            r#"
[plugin]
name = "policy_capture"
version = "0.1.0"
kind = "third_party"
entry = "init.lua"

[permissions]
routes = true
commands = true
admin = true

[policies]
scopes = ["api.notes.*", "cli.notes.run", "admin.notes.*"]
"#,
        )
        .unwrap();

        std::fs::write(
            dir.join("init.lua"),
            r#"
sushi.init = function()
    sushi.api.route("GET", "/api/notes", function()
        return "api"
    end, { policy = "api.notes.read" })

    sushi.cli.command("notes-run", "Run notes command", function()
        return "cli"
    end, { policy = "cli.notes.run" })

    sushi.admin.page("/admin/notes", "Notes", function()
        return "admin"
    end, { policy = "admin.notes.read" })
end
"#,
        )
        .unwrap();

        let plugins = LuaPlugin::scan_dir(tmp.path()).await.unwrap();
        let ctx = test_context().await;
        ctx.db
            .run_migrations(include_str!("../../../../migrations/001_init.sql"))
            .await
            .unwrap();
        ctx.db
            .run_migrations(include_str!("../../../../migrations/003_rbac.sql"))
            .await
            .unwrap();
        ctx.db
            .run_migrations(include_str!(
                "../../../../migrations/006_unified_policy_v2.sql"
            ))
            .await
            .unwrap();
        plugins[0].init(&ctx).await.unwrap();

        assert_eq!(
            ctx.plugins
                .api_route_policy("GET", "/api/notes")
                .await
                .as_deref(),
            Some("api.notes.read")
        );
        assert_eq!(
            ctx.plugins.cli_command_policy("notes-run").await.as_deref(),
            Some("cli.notes.run")
        );
        assert_eq!(
            ctx.plugins
                .admin_page_policy("/admin/notes")
                .await
                .as_deref(),
            Some("admin.notes.read")
        );

        let key_rows = ctx
            .db
            .query(
                r#"
                SELECT key
                FROM policy_keys
                WHERE key IN ('api.notes.read', 'cli.notes.run', 'admin.notes.read')
                ORDER BY key ASC
                "#,
                vec![],
            )
            .await
            .unwrap();
        assert_eq!(key_rows.len(), 3);

        let binding_rows = ctx
            .db
            .query(
                r#"
                SELECT
                    pb.surface,
                    pb.target_type,
                    pb.target_ref,
                    pb.method,
                    pb.path_pattern,
                    pb.command_name,
                    pk.key AS policy_key,
                    pb.owner_type,
                    pb.owner_id
                FROM policy_bindings pb
                JOIN policy_keys pk ON pk.id = pb.policy_key_id
                WHERE pb.owner_type = 'plugin'
                  AND pb.owner_id = 'policy_capture'
                ORDER BY pb.surface ASC, pb.target_type ASC, pb.target_ref ASC
                "#,
                vec![],
            )
            .await
            .unwrap();
        assert_eq!(binding_rows.len(), 3);

        let api_route_binding = binding_rows
            .iter()
            .find(|row| {
                row.get("surface")
                    .and_then(Value::as_str)
                    .map(|surface| surface == "api")
                    .unwrap_or(false)
            })
            .expect("missing api policy binding");
        assert_eq!(
            api_route_binding
                .get("target_type")
                .and_then(Value::as_str)
                .unwrap(),
            "http_route"
        );
        assert_eq!(
            api_route_binding
                .get("path_pattern")
                .and_then(Value::as_str)
                .unwrap(),
            "/api/notes"
        );
        assert_eq!(
            api_route_binding
                .get("policy_key")
                .and_then(Value::as_str)
                .unwrap(),
            "api.notes.read"
        );

        let admin_page_binding = binding_rows
            .iter()
            .find(|row| {
                row.get("surface")
                    .and_then(Value::as_str)
                    .map(|surface| surface == "admin")
                    .unwrap_or(false)
            })
            .expect("missing admin policy binding");
        assert_eq!(
            admin_page_binding
                .get("target_type")
                .and_then(Value::as_str)
                .unwrap(),
            "http_route"
        );
        assert_eq!(
            admin_page_binding
                .get("path_pattern")
                .and_then(Value::as_str)
                .unwrap(),
            "/admin/notes"
        );
        assert_eq!(
            admin_page_binding
                .get("policy_key")
                .and_then(Value::as_str)
                .unwrap(),
            "admin.notes.read"
        );

        let cli_binding = binding_rows
            .iter()
            .find(|row| {
                row.get("target_type")
                    .and_then(Value::as_str)
                    .map(|target_type| target_type == "cli_command")
                    .unwrap_or(false)
            })
            .expect("missing cli policy binding");
        assert_eq!(
            cli_binding
                .get("command_name")
                .and_then(Value::as_str)
                .unwrap(),
            "notes-run"
        );
        assert_eq!(
            cli_binding
                .get("policy_key")
                .and_then(Value::as_str)
                .unwrap(),
            "cli.notes.run"
        );
    }

    #[tokio::test]
    async fn plugin_load_without_policy_clears_stale_plugin_bindings() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("third_party").join("policy_capture");
        std::fs::create_dir_all(&dir).unwrap();

        std::fs::write(
            dir.join("plugin.toml"),
            r#"
[plugin]
name = "policy_capture"
version = "0.1.0"
kind = "third_party"
entry = "init.lua"

[permissions]
routes = true
commands = true
admin = true

[policies]
scopes = ["api.notes.*", "cli.notes.run", "admin.notes.*"]
"#,
        )
        .unwrap();

        std::fs::write(
            dir.join("init.lua"),
            r#"
sushi.init = function()
    sushi.api.route("GET", "/api/notes", function()
        return "api"
    end, { policy = "api.notes.read" })

    sushi.cli.command("notes-run", "Run notes command", function()
        return "cli"
    end, { policy = "cli.notes.run" })

    sushi.admin.page("/admin/notes", "Notes", function()
        return "admin"
    end, { policy = "admin.notes.read" })
end
"#,
        )
        .unwrap();

        let plugins = LuaPlugin::scan_dir(tmp.path()).await.unwrap();
        let ctx = test_context().await;
        ctx.db
            .run_migrations(include_str!("../../../../migrations/001_init.sql"))
            .await
            .unwrap();
        ctx.db
            .run_migrations(include_str!("../../../../migrations/003_rbac.sql"))
            .await
            .unwrap();
        ctx.db
            .run_migrations(include_str!(
                "../../../../migrations/006_unified_policy_v2.sql"
            ))
            .await
            .unwrap();
        plugins[0].init(&ctx).await.unwrap();

        std::fs::write(
            dir.join("init.lua"),
            r#"
sushi.init = function()
    sushi.api.route("GET", "/api/notes", function()
        return "api"
    end)

    sushi.cli.command("notes-run", "Run notes command", function()
        return "cli"
    end)

    sushi.admin.page("/admin/notes", "Notes", function()
        return "admin"
    end)
end
"#,
        )
        .unwrap();

        let plugins = LuaPlugin::scan_dir(tmp.path()).await.unwrap();
        plugins[0].init(&ctx).await.unwrap();

        assert_eq!(
            ctx.plugins.api_route_policy("GET", "/api/notes").await,
            None
        );
        assert_eq!(ctx.plugins.cli_command_policy("notes-run").await, None);
        assert_eq!(ctx.plugins.admin_page_policy("/admin/notes").await, None);

        let rows = ctx
            .db
            .query(
                r#"
                SELECT COUNT(*) AS count
                FROM policy_bindings pb
                WHERE pb.owner_type = 'plugin'
                  AND pb.owner_id = 'policy_capture'
                  AND (
                      pb.target_ref = '/api/notes'
                      OR pb.target_ref = '/admin/notes'
                      OR pb.target_ref = 'notes-run'
                  )
                "#,
                vec![],
            )
            .await
            .unwrap();

        let count = rows
            .first()
            .and_then(|row| row.get("count"))
            .and_then(Value::as_i64)
            .unwrap_or_default();
        assert_eq!(count, 0, "stale plugin policy bindings should be removed");
    }

    #[tokio::test]
    async fn test_lua_plugin_init_bad_manifest() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("official").join("bad_plugin");
        std::fs::create_dir_all(&dir).unwrap();

        // Invalid TOML
        std::fs::write(dir.join("plugin.toml"), "this is not valid toml [[[[").unwrap();

        let result = LuaPlugin::scan_dir(tmp.path()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn page_assets_resolve_bundle_then_page_assets() {
        let tmp = tempfile::tempdir().unwrap();
        let plugin_dir = tmp.path().join("asset_plugin");
        std::fs::create_dir_all(plugin_dir.join("web/static/pages")).unwrap();
        std::fs::write(plugin_dir.join("web/static/kv.js"), "console.log('kv')").unwrap();
        std::fs::write(
            plugin_dir.join("web/static/pages/extra.js"),
            "console.log('extra')",
        )
        .unwrap();

        let manifest: PluginManifest = toml::from_str(
            r#"
[plugin]
name = "asset_plugin"
version = "0.1.0"
kind = "third_party"
entry = "init.lua"

[permissions]
admin = true

[admin.assets.bundles.workspace]
js = ["kv.js"]
"#,
        )
        .unwrap();

        let resolved = resolve_page_assets_for_test(
            "third_party/asset_plugin",
            &manifest,
            &["workspace".to_string()],
            &["pages/extra.js".to_string()],
            &[],
            &plugin_dir.join("web/static"),
        )
        .unwrap();

        assert_eq!(
            resolved.js,
            vec![
                "/static/plugins/third_party/asset_plugin/kv.js".to_string(),
                "/static/plugins/third_party/asset_plugin/pages/extra.js".to_string()
            ]
        );
        assert!(resolved.css.is_empty());
    }

    #[tokio::test]
    async fn page_assets_fail_when_file_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let plugin_dir = tmp.path().join("asset_plugin");
        std::fs::create_dir_all(plugin_dir.join("web/static")).unwrap();

        let manifest: PluginManifest = toml::from_str(
            r#"
[plugin]
name = "asset_plugin"
version = "0.1.0"
kind = "third_party"
entry = "init.lua"

[permissions]
admin = true

[admin.assets.bundles.workspace]
js = ["missing.js"]
"#,
        )
        .unwrap();

        let err = resolve_page_assets_for_test(
            "third_party/asset_plugin",
            &manifest,
            &["workspace".to_string()],
            &[],
            &[],
            &plugin_dir.join("web/static"),
        )
        .unwrap_err();
        assert!(err.to_string().contains("missing.js"));
    }
}
