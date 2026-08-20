use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::{path::Component, path::Path};

use crate::context::PluginContext;
use crate::db::{DbGatewayError, DbPermission};
use crate::fs::{FileBrowserFsService, FsError};
use crate::plugin::Permissions;
use mlua::{Lua, LuaSerdeExt};

static HANDLER_COUNTER: AtomicU64 = AtomicU64::new(1);

fn next_handler_key() -> String {
    format!("h_{}", HANDLER_COUNTER.fetch_add(1, Ordering::Relaxed))
}

fn map_db_permission(permission: &crate::plugin::DatabasePermission) -> Option<DbPermission> {
    match permission {
        crate::plugin::DatabasePermission::ReadOnly => Some(DbPermission::ReadOnly),
        crate::plugin::DatabasePermission::Write => Some(DbPermission::Write),
        crate::plugin::DatabasePermission::Admin => Some(DbPermission::Admin),
        crate::plugin::DatabasePermission::None => None,
    }
}

fn lua_params(
    lua: &Lua,
    params: Option<mlua::Value>,
) -> Result<Vec<serde_json::Value>, mlua::Error> {
    match params {
        None | Some(mlua::Value::Nil) => Ok(Vec::new()),
        Some(value) => lua.from_value(value),
    }
}

fn map_db_gateway_error(err: DbGatewayError) -> mlua::Error {
    match err {
        DbGatewayError::PermissionDenied(message) => mlua::Error::RuntimeError(message),
        other => {
            mlua::Error::ExternalError(Arc::new(other) as Arc<dyn std::error::Error + Send + Sync>)
        }
    }
}

fn map_fs_error(err: FsError) -> mlua::Error {
    mlua::Error::RuntimeError(format!("{}: {err}", err.code()))
}

fn build_web_context(
    lua: &Lua,
    context: Option<mlua::Table>,
    static_url_prefix: &str,
) -> Result<serde_json::Value, mlua::Error> {
    let mut json_ctx = match context {
        Some(table) => lua.from_value(mlua::Value::Table(table))?,
        None => serde_json::Value::Object(serde_json::Map::new()),
    };

    match &mut json_ctx {
        serde_json::Value::Object(map) => {
            map.insert(
                "static_url_prefix".to_string(),
                serde_json::Value::String(static_url_prefix.to_string()),
            );
            Ok(json_ctx)
        }
        _ => Ok(serde_json::json!({
            "static_url_prefix": static_url_prefix,
            "data": json_ctx,
        })),
    }
}

fn validate_asset_path(path: &str, field: &str) -> Result<(), mlua::Error> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(mlua::Error::RuntimeError(format!(
            "sushi.web.page assets.{field} entries must be safe relative paths"
        )));
    }

    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") || trimmed.starts_with("//") {
        return Err(mlua::Error::RuntimeError(format!(
            "sushi.web.page assets.{field} entries must be safe relative paths"
        )));
    }

    if Path::new(trimmed).is_absolute() {
        return Err(mlua::Error::RuntimeError(format!(
            "sushi.web.page assets.{field} entries must be safe relative paths"
        )));
    }

    if trimmed.contains("..") {
        return Err(mlua::Error::RuntimeError(format!(
            "sushi.web.page assets.{field} entries must be safe relative paths"
        )));
    }

    for component in Path::new(trimmed).components() {
        if matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        ) {
            return Err(mlua::Error::RuntimeError(format!(
                "sushi.web.page assets.{field} entries must be safe relative paths"
            )));
        }
    }

    Ok(())
}

fn parse_asset_string_array(
    lua: &Lua,
    assets: &mlua::Table,
    field: &str,
    validate_path: bool,
) -> Result<Option<mlua::Table>, mlua::Error> {
    fn ensure_array_shape(values: &mlua::Table, field: &str) -> Result<usize, mlua::Error> {
        let len = values.raw_len();
        let mut entries = 0usize;
        for pair in values.pairs::<mlua::Value, mlua::Value>() {
            let (key, _) = pair?;
            entries += 1;
            match key {
                mlua::Value::Integer(index) if index >= 1 && (index as usize) <= len => {}
                _ => {
                    return Err(mlua::Error::RuntimeError(format!(
                        "sushi.web.page assets.{field} must be an array of strings"
                    )))
                }
            }
        }

        if entries != len {
            return Err(mlua::Error::RuntimeError(format!(
                "sushi.web.page assets.{field} must be an array of strings"
            )));
        }

        Ok(len)
    }

    match assets.get::<mlua::Value>(field)? {
        mlua::Value::Nil => Ok(None),
        mlua::Value::Table(values) => {
            let out = lua.create_table()?;
            let len = ensure_array_shape(&values, field)?;
            for idx in 1..=len {
                let value = match values.get::<mlua::Value>(idx)? {
                    mlua::Value::String(item) => item.to_str()?.to_string(),
                    _ => {
                        return Err(mlua::Error::RuntimeError(format!(
                            "sushi.web.page assets.{field} must be an array of strings"
                        )))
                    }
                };
                if validate_path {
                    validate_asset_path(&value, field)?;
                }
                out.set(idx, value)?;
            }
            Ok(Some(out))
        }
        _ => Err(mlua::Error::RuntimeError(format!(
            "sushi.web.page assets.{field} must be an array of strings"
        ))),
    }
}

fn parse_page_assets(lua: &Lua, assets: mlua::Table) -> Result<mlua::Table, mlua::Error> {
    let parsed = lua.create_table()?;

    if let Some(bundles) = parse_asset_string_array(lua, &assets, "bundles", false)? {
        parsed.set("bundles", bundles)?;
    }
    if let Some(js) = parse_asset_string_array(lua, &assets, "js", true)? {
        parsed.set("js", js)?;
    }
    if let Some(css) = parse_asset_string_array(lua, &assets, "css", true)? {
        parsed.set("css", css)?;
    }

    Ok(parsed)
}

fn parse_optional_policy(
    opts: &mlua::Table,
    api_name: &str,
) -> Result<Option<String>, mlua::Error> {
    match opts.get::<mlua::Value>("policy")? {
        mlua::Value::Nil => Ok(None),
        mlua::Value::String(value) => Ok(Some(value.to_str()?.to_string())),
        _ => Err(mlua::Error::RuntimeError(format!(
            "{api_name} opts.policy must be a string"
        ))),
    }
}

fn parse_optional_public(opts: &mlua::Table, api_name: &str) -> Result<Option<bool>, mlua::Error> {
    match opts.get::<mlua::Value>("public")? {
        mlua::Value::Nil => Ok(None),
        mlua::Value::Boolean(value) => Ok(Some(value)),
        _ => Err(mlua::Error::RuntimeError(format!(
            "{api_name} opts.public must be a boolean"
        ))),
    }
}

fn append_contract_entry(sushi: &mlua::Table, entry: mlua::Table) -> Result<(), mlua::Error> {
    let registry: mlua::Table = sushi.get("__contract_registry")?;
    registry.set(registry.raw_len() + 1, entry)
}

fn record_legacy_api_use(sushi: &mlua::Table, api: &str) -> Result<(), mlua::Error> {
    let diagnostics: mlua::Table = sushi.get("__deprecation_diagnostics")?;
    diagnostics.set(diagnostics.raw_len() + 1, api)
}

/// Inject the `sushi` global table into the Lua VM.
/// Only namespaces permitted by the plugin's permissions are injected.
pub async fn inject_plugin_api(
    lua: &Lua,
    ctx: &PluginContext,
    permissions: &Permissions,
) -> Result<(), mlua::Error> {
    let sushi = lua.create_table()?;
    sushi.set("__contract_registry", lua.create_table()?)?;
    sushi.set("__deprecation_diagnostics", lua.create_table()?)?;
    lua.globals().set("sushi", sushi.clone())?;
    crate::lua::injector::inject(lua, permissions.clone(), true)?;

    let sushi: mlua::Table = lua.globals().get("sushi")?;

    // sushi.__handlers — stores actual handler functions keyed by unique ID
    let handlers_table = lua.create_table()?;
    sushi.set("__handlers", handlers_table)?;

    // sushi.log -- always available
    {
        let log_service = ctx.logs();

        let log_table = lua.create_table()?;
        log_table.set(
            "info",
            lua.create_async_function({
                let log_svc = log_service.clone();
                move |_, msg: String| {
                    let log_svc = log_svc.clone();
                    async move {
                        tracing::info!("[lua] {msg}");
                        log_svc.info(&msg).await;
                        Ok(())
                    }
                }
            })?,
        )?;
        log_table.set(
            "warn",
            lua.create_async_function({
                let log_svc = log_service.clone();
                move |_, msg: String| {
                    let log_svc = log_svc.clone();
                    async move {
                        tracing::warn!("[lua] {msg}");
                        log_svc.warn(&msg).await;
                        Ok(())
                    }
                }
            })?,
        )?;
        log_table.set(
            "error",
            lua.create_async_function({
                let log_svc = log_service.clone();
                move |_, msg: String| {
                    let log_svc = log_svc.clone();
                    async move {
                        tracing::error!("[lua] {msg}");
                        log_svc.error(&msg).await;
                        Ok(())
                    }
                }
            })?,
        )?;
        sushi.set("log", log_table)?;
    }

    // sushi.api -- if routes permitted
    if permissions.routes {
        let api_table = lua.create_table()?;
        api_table.set(
            "route",
            lua.create_function(
                move |lua, (method, path, handler, opts): (String, String, mlua::Function, Option<mlua::Table>)| {
                    let handler_key = next_handler_key();
                    let sushi: mlua::Table = lua.globals().get("sushi")?;
                    record_legacy_api_use(&sushi, "sushi.api.route")?;
                    let handlers: mlua::Table = sushi.get("__handlers")?;
                    handlers.set(&*handler_key, handler)?;
                    let (policy, is_public) = match opts {
                        Some(table) => {
                            let policy = parse_optional_policy(&table, "sushi.api.route")?;
                            let is_public = parse_optional_public(&table, "sushi.api.route")?;
                            if policy.is_some() && is_public == Some(true) {
                                return Err(mlua::Error::RuntimeError(
                                    "sushi.api.route opts.policy cannot be combined with opts.public=true".to_string(),
                                ));
                            }
                            (policy, is_public.unwrap_or(false))
                        }
                        None => (None, false),
                    };

                    let entry = lua.create_table()?;
                    entry.set("surface", "api")?;
                    entry.set("method", method)?;
                    entry.set("path", path)?;
                    entry.set("handler", handler_key)?;
                    if let Some(policy) = policy {
                        entry.set("policy", policy)?;
                    }
                    if is_public {
                        entry.set("public", true)?;
                    }
                    append_contract_entry(&sushi, entry)
                },
            )?,
        )?;
        sushi.set("api", api_table)?;
    }

    // sushi.cli -- if commands permitted
    if permissions.commands {
        let cli_table = lua.create_table()?;
        cli_table.set(
            "command",
            lua.create_function(
                move |lua,
                      (name, desc, handler, opts): (
                    String,
                    String,
                    mlua::Function,
                    Option<mlua::Table>,
                )| {
                    let handler_key = next_handler_key();
                    let sushi: mlua::Table = lua.globals().get("sushi")?;
                    record_legacy_api_use(&sushi, "sushi.cli.command")?;
                    let handlers: mlua::Table = sushi.get("__handlers")?;
                    handlers.set(&*handler_key, handler)?;
                    let policy = match opts {
                        Some(table) => parse_optional_policy(&table, "sushi.cli.command")?,
                        None => None,
                    };

                    let entry = lua.create_table()?;
                    entry.set("surface", "cli")?;
                    entry.set("name", name)?;
                    entry.set("description", desc)?;
                    entry.set("handler", handler_key)?;
                    if let Some(policy) = policy {
                        entry.set("policy", policy)?;
                    }
                    append_contract_entry(&sushi, entry)
                },
            )?,
        )?;
        sushi.set("cli", cli_table)?;
    }

    // sushi.admin -- if admin permitted
    if permissions.admin {
        let admin_table = lua.create_table()?;
        admin_table.set(
            "page",
            lua.create_function(
                move |lua,
                      (path, title, handler, opts): (
                    String,
                    String,
                    mlua::Function,
                    Option<mlua::Table>,
                )| {
                    let handler_key = next_handler_key();
                    let sushi: mlua::Table = lua.globals().get("sushi")?;
                    record_legacy_api_use(&sushi, "sushi.admin.page")?;
                    let handlers: mlua::Table = sushi.get("__handlers")?;
                    handlers.set(&*handler_key, handler)?;
                    let policy = match opts {
                        Some(table) => parse_optional_policy(&table, "sushi.admin.page")?,
                        None => None,
                    };

                    let entry = lua.create_table()?;
                    entry.set("surface", "admin")?;
                    entry.set("path", path)?;
                    entry.set("title", title)?;
                    entry.set("handler", handler_key)?;
                    if let Some(policy) = policy {
                        entry.set("policy", policy)?;
                    }
                    append_contract_entry(&sushi, entry)
                },
            )?,
        )?;
        sushi.set("admin", admin_table)?;
    }

    if permissions.admin || permissions.routes {
        let static_url_prefix = {
            let cfg = ctx.config().get().await;
            cfg.web.static_url_prefix.clone()
        };
        let templates = ctx.templates();

        let web_table = lua.create_table()?;

        let render_templates = templates.clone();
        let render_prefix = static_url_prefix.clone();
        web_table.set(
            "render",
            lua.create_function(move |lua, (name, context): (String, Option<mlua::Table>)| {
                let json_ctx = build_web_context(lua, context, &render_prefix)?;
                render_templates
                    .render(&name, json_ctx)
                    .map_err(|e| mlua::Error::RuntimeError(format!("web render error: {e}")))
            })?,
        )?;

        let page_templates = templates.clone();
        let page_prefix = static_url_prefix.clone();
        let page_admin = permissions.admin;
        web_table.set(
            "page",
            lua.create_function(
                move |lua, (path, template_name, opts): (String, String, Option<mlua::Table>)| {
                    let sushi: mlua::Table = lua.globals().get("sushi")?;
                    record_legacy_api_use(&sushi, "sushi.web.page")?;
                    if !page_admin {
                        return Err(mlua::Error::RuntimeError(
                            "sushi.web.page requires admin permission".to_string(),
                        ));
                    }

                    let (title, context_table, assets_table, policy) = match opts {
                        Some(table) => {
                            let title = match table.get::<mlua::Value>("title")? {
                                mlua::Value::Nil => template_name.clone(),
                                mlua::Value::String(value) => value.to_str()?.to_string(),
                                _ => {
                                    return Err(mlua::Error::RuntimeError(
                                        "sushi.web.page opts.title must be a string".to_string(),
                                    ))
                                }
                            };
                            let context = match table.get::<mlua::Value>("context")? {
                                mlua::Value::Nil => None,
                                mlua::Value::Table(ctx) => Some(ctx),
                                _ => {
                                    return Err(mlua::Error::RuntimeError(
                                        "sushi.web.page opts.context must be a table".to_string(),
                                    ))
                                }
                            };
                            let assets = match table.get::<mlua::Value>("assets")? {
                                mlua::Value::Nil => None,
                                mlua::Value::Table(assets) => Some(parse_page_assets(lua, assets)?),
                                _ => {
                                    return Err(mlua::Error::RuntimeError(
                                        "sushi.web.page opts.assets must be a table".to_string(),
                                    ))
                                }
                            };
                            let policy = parse_optional_policy(&table, "sushi.web.page")?;
                            (title, context, assets, policy)
                        }
                        None => (template_name.clone(), None, None, None),
                    };

                    let json_ctx = build_web_context(lua, context_table, &page_prefix)?;
                    let handler_key = next_handler_key();

                    let handlers: mlua::Table = sushi.get("__handlers")?;

                    let handler_templates = page_templates.clone();
                    let handler_template_name = template_name.clone();
                    let handler_context = json_ctx.clone();
                    let handler = lua.create_async_function(move |_lua: Lua, ()| {
                        let templates = handler_templates.clone();
                        let template_name = handler_template_name.clone();
                        let context = handler_context.clone();
                        async move {
                            templates.render(&template_name, context).map_err(|e| {
                                mlua::Error::RuntimeError(format!("web render error: {e}"))
                            })
                        }
                    })?;

                    handlers.set(&*handler_key, handler)?;

                    let entry = lua.create_table()?;
                    entry.set("surface", "web")?;
                    entry.set("kind", "page")?;
                    entry.set("path", path)?;
                    entry.set("title", title)?;
                    entry.set("handler", handler_key)?;
                    if let Some(assets) = assets_table {
                        entry.set("assets", assets)?;
                    }
                    if let Some(policy) = policy {
                        entry.set("policy", policy)?;
                    }
                    append_contract_entry(&sushi, entry)
                },
            )?,
        )?;

        web_table.set(
            "json",
            lua.create_function(|lua, (status, data): (u16, mlua::Value)| {
                let json_data: serde_json::Value = lua.from_value(data)?;
                let envelope = serde_json::json!({
                    "__app_web_json": true,
                    "__sushi_web_json": true,
                    "status": status,
                    "body": json_data,
                });
                serde_json::to_string(&envelope)
                    .map_err(|e| mlua::Error::RuntimeError(format!("json encode error: {e}")))
            })?,
        )?;

        web_table.set(
            "download",
            lua.create_function(
                |_lua, (file_name, mime, body): (String, String, mlua::String)| {
                    let mut body_hex = String::with_capacity(body.as_bytes().len() * 2);
                    for byte in body.as_bytes().as_ref() {
                        use std::fmt::Write as _;
                        let _ = write!(&mut body_hex, "{byte:02x}");
                    }
                    let envelope = serde_json::json!({
                        "__app_web_download": true,
                        "__sushi_file_download": true,
                        "file_name": file_name,
                        "content_type": mime.clone(),
                        "mime": mime,
                        "body_hex": body_hex,
                    });
                    serde_json::to_string(&envelope)
                        .map_err(|e| mlua::Error::RuntimeError(format!("json encode error: {e}")))
                },
            )?,
        )?;

        sushi.set("web", web_table)?;
    }

    // sushi.db -- only when a database permission is granted
    if let Some(gateway_permission) = map_db_permission(&permissions.database) {
        let db_gateway = ctx
            .db()
            .ok_or_else(|| {
                mlua::Error::RuntimeError("database permission is required".to_string())
            })?
            .with_permission(gateway_permission);

        let db_table = lua.create_table()?;

        let db_query_gateway = db_gateway.clone();
        db_table.set(
            "query",
            lua.create_async_function(move |lua, (sql, params): (String, Option<mlua::Value>)| {
                let gateway = db_query_gateway.clone();
                async move {
                    let params = lua_params(&lua, params)?;
                    let rows = gateway
                        .query(&sql, params)
                        .await
                        .map_err(map_db_gateway_error)?;
                    lua.to_value(&rows)
                }
            })?,
        )?;

        let db_execute_gateway = db_gateway.clone();
        db_table.set(
            "execute",
            lua.create_async_function(move |lua, (sql, params): (String, Option<mlua::Value>)| {
                let gateway = db_execute_gateway.clone();
                async move {
                    let params = lua_params(&lua, params)?;
                    gateway
                        .execute(&sql, params)
                        .await
                        .map_err(map_db_gateway_error)?;
                    Ok(())
                }
            })?,
        )?;

        sushi.set("db", db_table)?;
    }

    // sushi.json -- always available
    {
        let json_table = lua.create_table()?;

        json_table.set(
            "encode",
            lua.create_function(|lua, value: mlua::Value| {
                let json_val: serde_json::Value = lua.from_value(value)?;
                serde_json::to_string(&json_val)
                    .map_err(|e| mlua::Error::RuntimeError(format!("json encode error: {e}")))
            })?,
        )?;

        json_table.set(
            "decode",
            lua.create_function(|lua, json_str: String| {
                let json_val: serde_json::Value = serde_json::from_str(&json_str)
                    .map_err(|e| mlua::Error::RuntimeError(format!("json decode error: {e}")))?;
                lua.to_value(&json_val)
            })?,
        )?;

        sushi.set("json", json_table)?;
    }

    // sushi.config -- profile entry configuration (read-only)
    {
        let config_table = lua.create_table()?;
        let plugin_config = ctx.config_value().clone();
        config_table.set(
            "get",
            lua.create_function(move |lua, key: String| {
                let value = plugin_config
                    .get(&key)
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
                if value.is_null() {
                    Ok(mlua::Value::Nil)
                } else {
                    lua.to_value(&value)
                }
            })?,
        )?;
        sushi.set("config", config_table)?;
    }

    // sushi.event -- always available
    {
        let event_bus = ctx.event();
        let event_table = lua.create_table()?;

        event_table.set(
            "on",
            lua.create_function(|lua, (event, callback): (String, mlua::Function)| {
                if event.trim().is_empty() {
                    return Err(mlua::Error::RuntimeError(
                        "sushi.event.on requires a non-empty event name".to_string(),
                    ));
                }
                let handler_key = next_handler_key();
                let sushi: mlua::Table = lua.globals().get("sushi")?;
                let handlers: mlua::Table = sushi.get("__handlers")?;
                handlers.set(&*handler_key, callback)?;
                let entry = lua.create_table()?;
                entry.set("surface", "event")?;
                entry.set("kind", "subscribe")?;
                entry.set("event", event)?;
                entry.set("handler_key", handler_key)?;
                append_contract_entry(&sushi, entry)
            })?,
        )?;

        let event_bus_emit = event_bus.clone();
        event_table.set(
            "emit",
            lua.create_async_function(move |_lua: Lua, (event, data): (String, mlua::Value)| {
                let bus = event_bus_emit.clone();
                async move {
                    // Convert Lua value to JSON
                    let json_data: serde_json::Value = match _lua.from_value(data.clone()) {
                        Ok(v) => v,
                        Err(_) => serde_json::Value::Null,
                    };

                    // Emit to event bus (async)
                    bus.emit(&event, &json_data).await;

                    tracing::debug!("Lua event emitted: {}", event);
                    Ok(())
                }
            })?,
        )?;
        sushi.set("event", event_table)?;
    }

    // sushi.task -- owner-scoped background task registration
    {
        let task_context = ctx.clone();
        let task_table = lua.create_table()?;
        task_table.set(
            "spawn",
            lua.create_async_function(
                move |lua, (name, callback): (String, mlua::Function)| {
                    let context = task_context.clone();
                    async move {
                        let callback = lua.create_registry_value(callback)?;
                        let runtime = lua.clone();
                        context
                            .register_task(name, move |mut cancellation| async move {
                                tokio::select! {
                                    _ = cancellation.cancelled() => {}
                                    result = async {
                                        let callback = runtime.registry_value::<mlua::Function>(&callback)?;
                                        callback.call_async::<()>(()).await
                                    } => {
                                        if let Err(error) = result {
                                            tracing::error!(error = %error, "lua background task failed");
                                        }
                                    }
                                }
                            })
                            .await
                            .map_err(mlua::Error::RuntimeError)
                    }
                },
            )?,
        )?;
        let interval_context = ctx.clone();
        task_table.set(
            "interval",
            lua.create_async_function(
                move |lua, (name, interval_ms, callback): (String, u64, mlua::Function)| {
                    let context = interval_context.clone();
                    async move {
                        if interval_ms == 0 {
                            return Err(mlua::Error::RuntimeError(
                                "sushi.task.interval requires interval_ms > 0".to_string(),
                            ));
                        }
                        let callback = lua.create_registry_value(callback)?;
                        let runtime = lua.clone();
                        context
                            .register_task(name, move |mut cancellation| async move {
                                let mut ticker = tokio::time::interval(
                                    std::time::Duration::from_millis(interval_ms),
                                );
                                loop {
                                    tokio::select! {
                                        _ = cancellation.cancelled() => break,
                                        _ = ticker.tick() => {
                                            let result = async {
                                                let callback = runtime.registry_value::<mlua::Function>(&callback)?;
                                                callback.call_async::<()>(()).await
                                            }.await;
                                            if let Err(error) = result {
                                                tracing::error!(error = %error, "lua interval task failed");
                                                break;
                                            }
                                        }
                                    }
                                }
                            })
                            .await
                            .map_err(mlua::Error::RuntimeError)
                    }
                },
            )?,
        )?;
        sushi.set("task", task_table)?;
    }

    // sushi.auth -- always available
    {
        let jwt = ctx.jwt();
        let auth_table = lua.create_table()?;
        auth_table.set(
            "verify_token",
            lua.create_function(move |lua, token: String| {
                match jwt.verify_token(&token) {
                    Ok(claims) => {
                        // Return claims as a Lua table
                        let claims_table = lua.create_table()?;
                        claims_table.set("sub", claims.sub.clone())?;
                        claims_table.set("username", claims.username.clone())?;
                        claims_table.set("role", claims.role.clone())?;
                        claims_table.set("token_type", claims.token_type.clone())?;
                        Ok(mlua::Value::Table(claims_table))
                    }
                    Err(e) => {
                        // Return nil on error (could also return error)
                        tracing::warn!("JWT verification failed in Lua: {}", e);
                        Ok(mlua::Value::Nil)
                    }
                }
            })?,
        )?;
        sushi.set("auth", auth_table)?;
    }

    lua.globals().set("sushi", sushi.clone())?;

    // app.* — shared namespace for cross-project plugin compatibility
    lua.globals().set("app", sushi.clone())?;

    // suxun.* — alias so suxun plugins work unchanged during migration
    lua.globals().set("suxun", sushi)?;

    Ok(())
}

/// Inject `sushi.fs` for file-browser capable plugins.
pub fn inject_sushi_fs(
    lua: &Lua,
    fs_service: Arc<FileBrowserFsService>,
) -> Result<(), mlua::Error> {
    let sushi: mlua::Table = lua.globals().get("sushi")?;
    let fs_table = lua.create_table()?;

    let route_prefix = fs_service.route_prefix().to_string();
    fs_table.set("route_prefix", route_prefix)?;

    let roots = fs_service.roots();
    fs_table.set(
        "roots",
        lua.create_function(move |lua, ()| lua.to_value(&roots))?,
    )?;

    let list_service = fs_service.clone();
    fs_table.set(
        "list",
        lua.create_async_function(move |lua, (root_id, rel_path): (String, String)| {
            let service = list_service.clone();
            async move {
                let entries = service
                    .list(&root_id, &rel_path)
                    .await
                    .map_err(map_fs_error)?;
                lua.to_value(&entries)
            }
        })?,
    )?;

    let read_text_service = fs_service.clone();
    fs_table.set(
        "read_text",
        lua.create_async_function(move |_lua, (root_id, rel_path): (String, String)| {
            let service = read_text_service.clone();
            async move {
                service
                    .read_text(&root_id, &rel_path)
                    .await
                    .map_err(map_fs_error)
            }
        })?,
    )?;

    let write_text_service = fs_service.clone();
    fs_table.set(
        "write_text",
        lua.create_async_function(
            move |_lua, (root_id, rel_path, content): (String, String, String)| {
                let service = write_text_service.clone();
                async move {
                    service
                        .write_text(&root_id, &rel_path, &content)
                        .await
                        .map_err(map_fs_error)
                }
            },
        )?,
    )?;

    let create_text_service = fs_service.clone();
    fs_table.set(
        "create_text",
        lua.create_async_function(
            move |_lua, (root_id, rel_path, content): (String, String, Option<String>)| {
                let service = create_text_service.clone();
                async move {
                    service
                        .create_text(&root_id, &rel_path, content.as_deref())
                        .await
                        .map_err(map_fs_error)
                }
            },
        )?,
    )?;

    let mkdir_service = fs_service.clone();
    fs_table.set(
        "mkdir",
        lua.create_async_function(move |_lua, (root_id, rel_path): (String, String)| {
            let service = mkdir_service.clone();
            async move {
                service
                    .mkdir(&root_id, &rel_path)
                    .await
                    .map_err(map_fs_error)
            }
        })?,
    )?;

    let rename_service = fs_service.clone();
    fs_table.set(
        "rename",
        lua.create_async_function(
            move |_lua, (root_id, from_path, to_path): (String, String, String)| {
                let service = rename_service.clone();
                async move {
                    service
                        .rename(&root_id, &from_path, &to_path)
                        .await
                        .map_err(map_fs_error)
                }
            },
        )?,
    )?;

    let delete_service = fs_service.clone();
    fs_table.set(
        "delete",
        lua.create_async_function(move |_lua, (root_id, rel_path): (String, String)| {
            let service = delete_service.clone();
            async move {
                service
                    .delete(&root_id, &rel_path)
                    .await
                    .map_err(map_fs_error)
            }
        })?,
    )?;

    let upload_service = fs_service.clone();
    fs_table.set(
        "write_upload",
        lua.create_async_function(
            move |_lua, (root_id, rel_path, content): (String, String, mlua::String)| {
                let service = upload_service.clone();
                async move {
                    service
                        .write_upload(&root_id, &rel_path, content.as_bytes().as_ref())
                        .await
                        .map_err(map_fs_error)
                }
            },
        )?,
    )?;

    let download_service = fs_service.clone();
    fs_table.set(
        "prepare_download",
        lua.create_async_function(move |lua, (root_id, rel_path): (String, String)| {
            let service = download_service.clone();
            async move {
                let ticket = service
                    .prepare_download(&root_id, &rel_path)
                    .await
                    .map_err(map_fs_error)?;
                lua.to_value(&ticket)
            }
        })?,
    )?;

    let read_download_service = fs_service.clone();
    fs_table.set(
        "read_download",
        lua.create_async_function(move |lua, (root_id, rel_path): (String, String)| {
            let service = read_download_service.clone();
            async move {
                let payload = service
                    .read_download(&root_id, &rel_path)
                    .await
                    .map_err(map_fs_error)?;
                let table = lua.create_table()?;
                table.set("root_id", payload.ticket.root_id)?;
                table.set("rel_path", payload.ticket.rel_path)?;
                table.set("file_name", payload.ticket.file_name)?;
                table.set("size", payload.ticket.size)?;
                table.set("content", lua.create_string(&payload.content)?)?;
                Ok(mlua::Value::Table(table))
            }
        })?,
    )?;

    sushi.set("fs", fs_table)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::jwt::JwtService;
    use crate::config::ConfigStore;
    use crate::context::SushiContext;
    use crate::lua::vm::create_sandboxed_vm;
    use crate::plugin::DatabasePermission;
    use crate::plugin::Permissions;
    use crate::storage::sqlite::SqliteStorage;
    use crate::storage::Storage;
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

    /// Build a minimal SushiContext for testing bindings.
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

    async fn inject_sushi_api(
        lua: &Lua,
        ctx: &SushiContext,
        permissions: &Permissions,
    ) -> Result<(), mlua::Error> {
        let plugin_context = ctx.plugin_context(permissions);
        inject_plugin_api(lua, &plugin_context, permissions).await
    }

    #[tokio::test]
    async fn test_inject_always_available_namespaces() {
        let lua = create_sandboxed_vm().unwrap();
        let ctx = test_context().await;
        let permissions = Permissions::default();

        inject_sushi_api(&lua, &ctx, &permissions).await.unwrap();

        let sushi: mlua::Table = lua.globals().get("sushi").unwrap();

        // Always-available namespaces
        assert!(sushi.contains_key("log").unwrap());
        assert!(sushi.contains_key("config").unwrap());
        assert!(sushi.contains_key("event").unwrap());
        assert!(sushi.contains_key("auth").unwrap());
        // __handlers table always created
        assert!(sushi.contains_key("__handlers").unwrap());
    }

    #[tokio::test]
    async fn test_inject_routes_permission() {
        let lua = create_sandboxed_vm().unwrap();
        let ctx = test_context().await;
        let mut permissions = Permissions::default();
        permissions.routes = true;

        inject_sushi_api(&lua, &ctx, &permissions).await.unwrap();

        let sushi: mlua::Table = lua.globals().get("sushi").unwrap();
        assert!(sushi.contains_key("api").unwrap());
        assert!(sushi.contains_key("__contract_registry").unwrap());
    }

    #[tokio::test]
    async fn test_inject_commands_permission() {
        let lua = create_sandboxed_vm().unwrap();
        let ctx = test_context().await;
        let mut permissions = Permissions::default();
        permissions.commands = true;

        inject_sushi_api(&lua, &ctx, &permissions).await.unwrap();

        let sushi: mlua::Table = lua.globals().get("sushi").unwrap();
        assert!(sushi.contains_key("cli").unwrap());
        assert!(sushi.contains_key("__contract_registry").unwrap());
    }

    #[tokio::test]
    async fn test_inject_admin_permission() {
        let lua = create_sandboxed_vm().unwrap();
        let ctx = test_context().await;
        let mut permissions = Permissions::default();
        permissions.admin = true;

        inject_sushi_api(&lua, &ctx, &permissions).await.unwrap();

        let sushi: mlua::Table = lua.globals().get("sushi").unwrap();
        assert!(sushi.contains_key("admin").unwrap());
        assert!(sushi.contains_key("__contract_registry").unwrap());
    }

    #[tokio::test]
    async fn test_lua_db_injected_with_database_permission() {
        let lua = create_sandboxed_vm().unwrap();
        let ctx = test_context().await;

        let permissions = Permissions::default();
        inject_sushi_api(&lua, &ctx, &permissions).await.unwrap();
        let sushi: mlua::Table = lua.globals().get("sushi").unwrap();
        assert!(!sushi.contains_key("db").unwrap());

        let lua = create_sandboxed_vm().unwrap();
        let mut permissions = Permissions::default();
        permissions.database = DatabasePermission::ReadOnly;
        inject_sushi_api(&lua, &ctx, &permissions).await.unwrap();
        let sushi: mlua::Table = lua.globals().get("sushi").unwrap();
        assert!(sushi.contains_key("db").unwrap());
    }

    #[tokio::test]
    async fn test_lua_db_execute_denied_readonly() {
        let lua = create_sandboxed_vm().unwrap();
        let ctx = test_context().await;
        let mut permissions = Permissions::default();
        permissions.database = DatabasePermission::ReadOnly;

        inject_sushi_api(&lua, &ctx, &permissions).await.unwrap();

        let result: mlua::Result<mlua::Value> = lua
            .load("return sushi.db.execute('CREATE TABLE denied (id INTEGER)')")
            .eval_async()
            .await;

        match result {
            Err(err) => {
                let message = err.to_string();
                assert!(message.contains("does not allow"));
            }
            Ok(_) => panic!("expected permission error"),
        }
    }

    #[tokio::test]
    async fn test_lua_db_query_params_handling() {
        let lua = create_sandboxed_vm().unwrap();
        let ctx = test_context().await;
        let mut permissions = Permissions::default();
        permissions.database = DatabasePermission::ReadOnly;

        Storage::execute(
            &*ctx.db,
            "CREATE TABLE test_items (id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
            vec![],
        )
        .await
        .unwrap();
        Storage::execute(
            &*ctx.db,
            "INSERT INTO test_items (name) VALUES (?1)",
            vec![serde_json::Value::String("ok".to_string())],
        )
        .await
        .unwrap();

        inject_sushi_api(&lua, &ctx, &permissions).await.unwrap();

        let rows_value: mlua::Value = lua
            .load("return sushi.db.query('SELECT name FROM test_items ORDER BY id')")
            .eval_async()
            .await
            .unwrap();
        let rows: Vec<serde_json::Value> = lua.from_value(rows_value).unwrap();
        assert_eq!(rows.len(), 1);

        let rows_with_nil_value: mlua::Value = lua
            .load("return sushi.db.query('SELECT name FROM test_items ORDER BY id', nil)")
            .eval_async()
            .await
            .unwrap();
        let rows_with_nil: Vec<serde_json::Value> = lua.from_value(rows_with_nil_value).unwrap();
        assert_eq!(rows_with_nil.len(), 1);

        let rows_with_params_value: mlua::Value = lua
            .load("return sushi.db.query('SELECT name FROM test_items WHERE name = ?1', { 'ok' })")
            .eval_async()
            .await
            .unwrap();
        let rows_with_params: Vec<serde_json::Value> =
            lua.from_value(rows_with_params_value).unwrap();
        assert_eq!(rows_with_params.len(), 1);

        let invalid_params: mlua::Result<mlua::Value> = lua
            .load("return sushi.db.query('SELECT name FROM test_items WHERE name = ?1', 'bad')")
            .eval_async()
            .await;
        assert!(invalid_params.is_err());
    }

    #[tokio::test]
    async fn test_no_api_without_routes_permission() {
        let lua = create_sandboxed_vm().unwrap();
        let ctx = test_context().await;
        let permissions = Permissions::default();

        inject_sushi_api(&lua, &ctx, &permissions).await.unwrap();

        let sushi: mlua::Table = lua.globals().get("sushi").unwrap();
        assert!(!sushi.contains_key("api").unwrap());
        assert!(!sushi.contains_key("cli").unwrap());
        assert!(!sushi.contains_key("admin").unwrap());
    }

    #[tokio::test]
    async fn test_log_functions_callable() {
        let lua = create_sandboxed_vm().unwrap();
        let ctx = test_context().await;
        let permissions = Permissions::default();

        inject_sushi_api(&lua, &ctx, &permissions).await.unwrap();

        // Should not error — logging functions just trace
        lua.load("sushi.log.info('hello from lua')").exec().unwrap();
        lua.load("sushi.log.warn('warning')").exec().unwrap();
        lua.load("sushi.log.error('error')").exec().unwrap();
    }

    #[tokio::test]
    async fn test_lua_web_render_renders_template() {
        let lua = create_sandboxed_vm().unwrap();
        let ctx = test_context().await;
        let mut permissions = Permissions::default();
        permissions.admin = true;

        let templates_root = ctx._templates_dir.path().join("admin");
        std::fs::create_dir_all(&templates_root).unwrap();
        std::fs::write(
            templates_root.join("login.html"),
            "Sushi Admin {{ title }} {{ static_url_prefix }}",
        )
        .unwrap();

        inject_sushi_api(&lua, &ctx, &permissions).await.unwrap();

        let rendered: String = lua
            .load(r#"return sushi.web.render("admin/login.html", { title = "Login" })"#)
            .eval()
            .unwrap();

        let static_url_prefix = {
            let cfg = ctx.config.get().await;
            cfg.web.static_url_prefix.clone()
        };

        assert!(!static_url_prefix.is_empty());
        assert!(rendered.contains("Sushi Admin"));
        let escaped_prefix = static_url_prefix.replace('/', "&#x2f;");
        assert!(rendered.contains(&escaped_prefix));
    }

    #[tokio::test]
    async fn test_lua_web_available_with_routes_permission() {
        let lua = create_sandboxed_vm().unwrap();
        let ctx = test_context().await;
        let mut permissions = Permissions::default();
        permissions.routes = true;

        inject_sushi_api(&lua, &ctx, &permissions).await.unwrap();

        let sushi: mlua::Table = lua.globals().get("sushi").unwrap();
        assert!(sushi.contains_key("web").unwrap());
    }

    #[tokio::test]
    async fn test_lua_web_page_registers_and_renders_template() {
        let lua = create_sandboxed_vm().unwrap();
        let ctx = test_context().await;
        let mut permissions = Permissions::default();
        permissions.admin = true;

        let templates_root = ctx._templates_dir.path().join("admin");
        std::fs::create_dir_all(&templates_root).unwrap();
        std::fs::write(templates_root.join("page.html"), "Page {{ name }}").unwrap();

        inject_sushi_api(&lua, &ctx, &permissions).await.unwrap();

        lua.load(
            r#"sushi.web.page("/admin/lua", "admin/page.html", { title = "Lua Page", context = { name = "Lua" } })"#,
        )
        .exec()
        .unwrap();

        let sushi: mlua::Table = lua.globals().get("sushi").unwrap();
        let registry: mlua::Table = sushi.get("__contract_registry").unwrap();
        assert_eq!(registry.raw_len(), 1);
        let entry: mlua::Table = registry.get(1).unwrap();
        assert_eq!(entry.get::<String>("path").unwrap(), "/admin/lua");
        assert_eq!(entry.get::<String>("title").unwrap(), "Lua Page");

        let handler_key: String = entry.get("handler").unwrap();
        let handlers: mlua::Table = sushi.get("__handlers").unwrap();
        let handler: mlua::Function = handlers.get(handler_key).unwrap();
        let rendered: String = handler.call_async(()).await.unwrap();
        assert!(rendered.contains("Page Lua"));
    }

    #[tokio::test]
    async fn test_lua_web_page_accepts_assets_and_stores_entries() {
        let lua = create_sandboxed_vm().unwrap();
        let ctx = test_context().await;
        let mut permissions = Permissions::default();
        permissions.admin = true;

        inject_sushi_api(&lua, &ctx, &permissions).await.unwrap();

        lua.load(
            r#"sushi.web.page("/admin/assets", "admin/page.html", {
                title = "Asset Page",
                assets = {
                    bundles = { "workspace", "charts" },
                    js = { "pages/workspace.js", "vendor/charts.js" },
                    css = { "pages/workspace.css" }
                }
            })"#,
        )
        .exec()
        .unwrap();

        let sushi: mlua::Table = lua.globals().get("sushi").unwrap();
        let registry: mlua::Table = sushi.get("__contract_registry").unwrap();
        let entry: mlua::Table = registry.get(1).unwrap();
        let assets: mlua::Table = entry.get("assets").unwrap();

        let bundles: mlua::Table = assets.get("bundles").unwrap();
        assert_eq!(bundles.get::<String>(1).unwrap(), "workspace");
        assert_eq!(bundles.get::<String>(2).unwrap(), "charts");

        let js: mlua::Table = assets.get("js").unwrap();
        assert_eq!(js.get::<String>(1).unwrap(), "pages/workspace.js");
        assert_eq!(js.get::<String>(2).unwrap(), "vendor/charts.js");

        let css: mlua::Table = assets.get("css").unwrap();
        assert_eq!(css.get::<String>(1).unwrap(), "pages/workspace.css");
    }

    #[tokio::test]
    async fn test_lua_web_page_rejects_invalid_asset_path() {
        let lua = create_sandboxed_vm().unwrap();
        let ctx = test_context().await;
        let mut permissions = Permissions::default();
        permissions.admin = true;

        inject_sushi_api(&lua, &ctx, &permissions).await.unwrap();

        let result: mlua::Result<()> = lua
            .load(
                r#"sushi.web.page("/admin/assets", "admin/page.html", {
                    assets = { js = { "../escape.js" } }
                })"#,
            )
            .exec();
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("safe relative paths"));
    }

    #[tokio::test]
    async fn test_lua_web_page_rejects_non_array_bundles() {
        let lua = create_sandboxed_vm().unwrap();
        let ctx = test_context().await;
        let mut permissions = Permissions::default();
        permissions.admin = true;

        inject_sushi_api(&lua, &ctx, &permissions).await.unwrap();

        let result: mlua::Result<()> = lua
            .load(
                r#"sushi.web.page("/admin/assets", "admin/page.html", {
                    assets = { bundles = { workspace = "main" } }
                })"#,
            )
            .exec();
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("assets.bundles must be an array of strings"));
    }

    #[tokio::test]
    async fn test_lua_web_page_rejects_non_string_js_element() {
        let lua = create_sandboxed_vm().unwrap();
        let ctx = test_context().await;
        let mut permissions = Permissions::default();
        permissions.admin = true;

        inject_sushi_api(&lua, &ctx, &permissions).await.unwrap();

        let result: mlua::Result<()> = lua
            .load(
                r#"sushi.web.page("/admin/assets", "admin/page.html", {
                    assets = { js = { "pages/workspace.js", 42 } }
                })"#,
            )
            .exec();
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("assets.js must be an array of strings"));
    }

    #[tokio::test]
    async fn test_lua_web_page_rejects_absolute_js_asset_path() {
        let lua = create_sandboxed_vm().unwrap();
        let ctx = test_context().await;
        let mut permissions = Permissions::default();
        permissions.admin = true;

        inject_sushi_api(&lua, &ctx, &permissions).await.unwrap();

        let result: mlua::Result<()> = lua
            .load(
                r#"sushi.web.page("/admin/assets", "admin/page.html", {
                    assets = { js = { "/absolute.js" } }
                })"#,
            )
            .exec();
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("safe relative paths"));
    }

    #[tokio::test]
    async fn test_lua_web_page_rejects_url_js_asset_path() {
        let lua = create_sandboxed_vm().unwrap();
        let ctx = test_context().await;
        let mut permissions = Permissions::default();
        permissions.admin = true;

        inject_sushi_api(&lua, &ctx, &permissions).await.unwrap();

        let result: mlua::Result<()> = lua
            .load(
                r#"sushi.web.page("/admin/assets", "admin/page.html", {
                    assets = { js = { "https://cdn.example.com/app.js" } }
                })"#,
            )
            .exec();
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("safe relative paths"));
    }

    #[tokio::test]
    async fn test_lua_web_page_rejects_whitespace_js_asset_path() {
        let lua = create_sandboxed_vm().unwrap();
        let ctx = test_context().await;
        let mut permissions = Permissions::default();
        permissions.admin = true;

        inject_sushi_api(&lua, &ctx, &permissions).await.unwrap();

        let result: mlua::Result<()> = lua
            .load(
                r#"sushi.web.page("/admin/assets", "admin/page.html", {
                    assets = { js = { "   " } }
                })"#,
            )
            .exec();
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("safe relative paths"));
    }

    #[tokio::test]
    async fn test_lua_web_json_envelope_shape() {
        let lua = create_sandboxed_vm().unwrap();
        let ctx = test_context().await;
        let mut permissions = Permissions::default();
        permissions.routes = true;

        inject_sushi_api(&lua, &ctx, &permissions).await.unwrap();

        let rendered: String = lua
            .load(r#"return sushi.web.json(201, { ok = true })"#)
            .eval()
            .unwrap();

        let value: serde_json::Value = serde_json::from_str(&rendered).unwrap();
        assert_eq!(
            value.get("__sushi_web_json").and_then(|v| v.as_bool()),
            Some(true)
        );
        assert_eq!(value.get("status").and_then(|v| v.as_u64()), Some(201));
        assert_eq!(
            value
                .get("body")
                .and_then(|v| v.get("ok"))
                .and_then(|v| v.as_bool()),
            Some(true)
        );
    }

    #[tokio::test]
    async fn test_lua_web_page_errors_without_admin_permission() {
        let lua = create_sandboxed_vm().unwrap();
        let ctx = test_context().await;
        let mut permissions = Permissions::default();
        permissions.routes = true;

        inject_sushi_api(&lua, &ctx, &permissions).await.unwrap();

        let result: mlua::Result<()> = lua
            .load(r#"sushi.web.page("/admin/lua", "admin/page.html")"#)
            .exec();
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_lua_web_static_url_prefix_not_overridable() {
        let lua = create_sandboxed_vm().unwrap();
        let ctx = test_context().await;
        let mut permissions = Permissions::default();
        permissions.admin = true;

        let templates_root = ctx._templates_dir.path().join("admin");
        std::fs::create_dir_all(&templates_root).unwrap();
        std::fs::write(
            templates_root.join("override.html"),
            "Prefix {{ static_url_prefix }}",
        )
        .unwrap();

        inject_sushi_api(&lua, &ctx, &permissions).await.unwrap();

        let rendered: String = lua
            .load(
                r#"return sushi.web.render("admin/override.html", { static_url_prefix = "/evil" })"#,
            )
            .eval()
            .unwrap();

        let static_url_prefix = {
            let cfg = ctx.config.get().await;
            cfg.web.static_url_prefix.clone()
        };

        let escaped_prefix = static_url_prefix.replace('/', "&#x2f;");
        let escaped_override = "/evil".replace('/', "&#x2f;");
        assert!(rendered.contains(&escaped_prefix));
        assert!(!rendered.contains(&escaped_override));
    }

    #[tokio::test]
    async fn test_api_route_registration() {
        let lua = create_sandboxed_vm().unwrap();
        let ctx = test_context().await;
        let mut permissions = Permissions::default();
        permissions.routes = true;

        inject_sushi_api(&lua, &ctx, &permissions).await.unwrap();

        lua.load("sushi.api.route('GET', '/api/test', function() return 'ok' end)")
            .exec()
            .unwrap();
        lua.load("sushi.api.route('POST', '/api/items', function() return 'created' end)")
            .exec()
            .unwrap();

        let registry: mlua::Table = lua
            .globals()
            .get::<mlua::Table>("sushi")
            .unwrap()
            .get("__contract_registry")
            .unwrap();
        assert_eq!(registry.raw_len(), 2);

        let first: mlua::Table = registry.get(1).unwrap();
        let method: String = first.get("method").unwrap();
        let path: String = first.get("path").unwrap();
        let handler_key: String = first.get("handler").unwrap();
        assert_eq!(method, "GET");
        assert_eq!(path, "/api/test");
        assert!(!first.contains_key("policy").unwrap());
        assert!(handler_key.starts_with("h_"));

        // Verify handler function is stored in sushi.__handlers
        let handlers: mlua::Table = lua
            .globals()
            .get::<mlua::Table>("sushi")
            .unwrap()
            .get("__handlers")
            .unwrap();
        let _handler: mlua::Function = handlers.get(&*handler_key).unwrap();
    }

    #[tokio::test]
    async fn test_registration_entries_include_policy_when_opts_provided() {
        let lua = create_sandboxed_vm().unwrap();
        let ctx = test_context().await;
        let permissions = Permissions {
            routes: true,
            commands: true,
            admin: true,
            database: DatabasePermission::None,
        };

        let templates_root = ctx._templates_dir.path().join("admin");
        std::fs::create_dir_all(&templates_root).unwrap();
        std::fs::write(templates_root.join("policy.html"), "Policy page").unwrap();

        inject_sushi_api(&lua, &ctx, &permissions).await.unwrap();

        lua.load(
            r#"sushi.api.route("GET", "/api/policy", function() return "ok" end, { policy = "api.policy.read" })"#,
        )
        .exec()
        .unwrap();
        lua.load(
            r#"sushi.cli.command("policy-check", "check policy", function() return true end, { policy = "cli.policy.run" })"#,
        )
        .exec()
        .unwrap();
        lua.load(
            r#"sushi.web.page("/admin/policy-web", "admin/policy.html", { policy = "admin.page.web.read" })"#,
        )
        .exec()
        .unwrap();
        lua.load(
            r#"sushi.admin.page("/admin/policy-admin", "Policy Admin", function() return "ok" end, { policy = "admin.page.direct.read" })"#,
        )
        .exec()
        .unwrap();

        let sushi: mlua::Table = lua.globals().get("sushi").unwrap();

        let registry: mlua::Table = sushi.get("__contract_registry").unwrap();
        let route_entry: mlua::Table = registry.get(1).unwrap();
        assert_eq!(
            route_entry.get::<String>("policy").unwrap(),
            "api.policy.read"
        );

        let command_entry: mlua::Table = registry.get(2).unwrap();
        assert_eq!(
            command_entry.get::<String>("policy").unwrap(),
            "cli.policy.run"
        );

        let web_page_entry: mlua::Table = registry.get(3).unwrap();
        let admin_page_entry: mlua::Table = registry.get(4).unwrap();
        assert_eq!(
            web_page_entry.get::<String>("policy").unwrap(),
            "admin.page.web.read"
        );
        assert_eq!(
            admin_page_entry.get::<String>("policy").unwrap(),
            "admin.page.direct.read"
        );
    }

    #[tokio::test]
    async fn test_api_route_registration_public_flag_when_enabled() {
        let lua = create_sandboxed_vm().unwrap();
        let ctx = test_context().await;
        let mut permissions = Permissions::default();
        permissions.routes = true;

        inject_sushi_api(&lua, &ctx, &permissions).await.unwrap();

        lua.load(
            r#"sushi.api.route("GET", "/api/public", function() return "ok" end, { public = true })"#,
        )
        .exec()
        .unwrap();

        let registry: mlua::Table = lua
            .globals()
            .get::<mlua::Table>("sushi")
            .unwrap()
            .get("__contract_registry")
            .unwrap();
        let first: mlua::Table = registry.get(1).unwrap();
        assert_eq!(first.get::<bool>("public").unwrap(), true);
    }

    #[tokio::test]
    async fn test_api_route_registration_public_requires_boolean() {
        let lua = create_sandboxed_vm().unwrap();
        let ctx = test_context().await;
        let mut permissions = Permissions::default();
        permissions.routes = true;

        inject_sushi_api(&lua, &ctx, &permissions).await.unwrap();

        let result: mlua::Result<()> = lua
            .load(
                r#"sushi.api.route("GET", "/api/public", function() return "ok" end, { public = "yes" })"#,
            )
            .exec();
        let err = result.expect_err("public should require boolean");
        assert!(err
            .to_string()
            .contains("sushi.api.route opts.public must be a boolean"));
    }

    #[tokio::test]
    async fn test_api_route_registration_rejects_policy_and_public_true() {
        let lua = create_sandboxed_vm().unwrap();
        let ctx = test_context().await;
        let mut permissions = Permissions::default();
        permissions.routes = true;

        inject_sushi_api(&lua, &ctx, &permissions).await.unwrap();

        let result: mlua::Result<()> = lua
            .load(
                r#"sushi.api.route("GET", "/api/public", function() return "ok" end, { policy = "api.public.read", public = true })"#,
            )
            .exec();
        let err = result.expect_err("policy/public conflict should be rejected");
        assert!(err
            .to_string()
            .contains("sushi.api.route opts.policy cannot be combined with opts.public=true"));
    }
}
