use crate::lua::contract::schema::api::ApiRouteContract;
use crate::lua::registry::CapabilitySnapshot;
use crate::plugin::PluginError;
use std::sync::atomic::{AtomicU64, Ordering};

static CONTRACT_HANDLER_COUNTER: AtomicU64 = AtomicU64::new(1);

pub fn snapshot_from_lua(
    lua: &mlua::Lua,
    raw_registry: mlua::Table,
) -> Result<CapabilitySnapshot, PluginError> {
    let sushi: mlua::Table = lua
        .globals()
        .get("sushi")
        .map_err(|e| PluginError::InitFailed(format!("missing sushi global: {e}")))?;
    let handlers: mlua::Table = sushi
        .get("__handlers")
        .map_err(|e| PluginError::InitFailed(format!("missing sushi.__handlers table: {e}")))?;

    let mut routes = Vec::new();
    let len = raw_registry.raw_len();
    for index in 1..=len {
        let value = raw_registry.get::<mlua::Value>(index).map_err(|e| {
            PluginError::InitFailed(format!(
                "invalid contract registry entry at index {index}: {e}"
            ))
        })?;

        let entry = match value {
            mlua::Value::Table(table) => table,
            mlua::Value::Nil => continue,
            _ => {
                return Err(PluginError::InitFailed(format!(
                    "contract registry entry {index} must be a table"
                )))
            }
        };

        let surface = entry
            .get::<Option<String>>("surface")
            .map_err(|e| {
                PluginError::InitFailed(format!(
                    "contract registry entry {index} has invalid surface: {e}"
                ))
            })?
            .unwrap_or_default();
        if surface != "api" {
            continue;
        }

        let method = entry.get::<String>("method").map_err(|e| {
            PluginError::InitFailed(format!(
                "contract registry api entry {index} has invalid method: {e}"
            ))
        })?;
        let path = entry.get::<String>("path").map_err(|e| {
            PluginError::InitFailed(format!(
                "contract registry api entry {index} has invalid path: {e}"
            ))
        })?;
        let policy = parse_optional_string(&entry, "policy", index, "api")?;
        let public = parse_optional_bool(&entry, "public", index, "api")?.unwrap_or(false);

        let handler_from_key = parse_optional_string(&entry, "handler_key", index, "api")?
            .filter(|value| !value.is_empty());
        let handler_key = resolve_handler_key(&handlers, &entry, handler_from_key, index)?;

        routes.push(ApiRouteContract {
            method,
            path,
            handler_key,
            policy,
            public,
        });
    }

    Ok(CapabilitySnapshot { api_routes: routes })
}

fn parse_optional_string(
    entry: &mlua::Table,
    field: &str,
    index: usize,
    surface: &str,
) -> Result<Option<String>, PluginError> {
    match entry.get::<mlua::Value>(field).map_err(|e| {
        PluginError::InitFailed(format!(
            "contract registry {surface} entry {index} has invalid {field}: {e}"
        ))
    })? {
        mlua::Value::Nil => Ok(None),
        mlua::Value::String(value) => Ok(Some(
            value
                .to_str()
                .map_err(|e| {
                    PluginError::InitFailed(format!(
                        "contract registry {surface} entry {index} has invalid {field}: {e}"
                    ))
                })?
                .to_string(),
        )),
        _ => Err(PluginError::InitFailed(format!(
            "contract registry {surface} entry {index} field '{field}' must be a string"
        ))),
    }
}

fn parse_optional_bool(
    entry: &mlua::Table,
    field: &str,
    index: usize,
    surface: &str,
) -> Result<Option<bool>, PluginError> {
    match entry.get::<mlua::Value>(field).map_err(|e| {
        PluginError::InitFailed(format!(
            "contract registry {surface} entry {index} has invalid {field}: {e}"
        ))
    })? {
        mlua::Value::Nil => Ok(None),
        mlua::Value::Boolean(value) => Ok(Some(value)),
        _ => Err(PluginError::InitFailed(format!(
            "contract registry {surface} entry {index} field '{field}' must be a boolean"
        ))),
    }
}

fn resolve_handler_key(
    handlers: &mlua::Table,
    entry: &mlua::Table,
    handler_key: Option<String>,
    index: usize,
) -> Result<String, PluginError> {
    match entry.get::<mlua::Value>("handler").map_err(|e| {
        PluginError::InitFailed(format!(
            "contract registry api entry {index} has invalid handler: {e}"
        ))
    })? {
        mlua::Value::Nil => handler_key.ok_or_else(|| {
            PluginError::InitFailed(format!(
                "contract registry api entry {index} must include handler or handler_key"
            ))
        }),
        mlua::Value::Function(handler_fn) => {
            let resolved_key =
                handler_key.unwrap_or_else(|| format!("contract_handler_{}", next_handler_id()));
            handlers
                .set(resolved_key.as_str(), handler_fn)
                .map_err(|e| {
                    PluginError::InitFailed(format!(
                        "failed to cache api handler for contract registry entry {index}: {e}"
                    ))
                })?;
            Ok(resolved_key)
        }
        mlua::Value::String(value) => Ok(value
            .to_str()
            .map_err(|e| {
                PluginError::InitFailed(format!(
                    "contract registry api entry {index} has invalid handler key: {e}"
                ))
            })?
            .to_string()),
        _ => Err(PluginError::InitFailed(format!(
            "contract registry api entry {index} field 'handler' must be a function or string"
        ))),
    }
}

fn next_handler_id() -> u64 {
    CONTRACT_HANDLER_COUNTER.fetch_add(1, Ordering::Relaxed)
}
