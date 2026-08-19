use crate::lua::contract::schema::admin::AdminPageContract;
use crate::plugin::PluginError;
use std::sync::atomic::{AtomicU64, Ordering};

static CONTRACT_ADMIN_HANDLER_COUNTER: AtomicU64 = AtomicU64::new(1);

pub fn snapshot_from_lua(
    lua: &mlua::Lua,
    raw_registry: mlua::Table,
) -> Result<Vec<AdminPageContract>, PluginError> {
    let app: mlua::Table = lua
        .globals()
        .get("app")
        .map_err(|e| PluginError::InitFailed(format!("missing app global: {e}")))?;
    let handlers: mlua::Table = app
        .get("__handlers")
        .map_err(|e| PluginError::InitFailed(format!("missing app.__handlers table: {e}")))?;
    let mut pages = Vec::new();

    for index in 1..=raw_registry.raw_len() {
        let Some(entry) = registry_entry(&raw_registry, index)? else {
            continue;
        };
        if entry.get::<Option<String>>("surface").map_err(|e| {
            PluginError::InitFailed(format!(
                "contract registry entry {index} has invalid surface: {e}"
            ))
        })? != Some("admin".to_string())
        {
            continue;
        }

        let path = required_string(&entry, "path", index)?;
        let title = required_string(&entry, "title", index)?;
        let policy = optional_string(&entry, "policy", index)?;
        let handler_key = resolve_handler_key(&handlers, &entry, index)?;
        let (bundles, js, css) = parse_assets(&entry, index)?;
        pages.push(AdminPageContract {
            path,
            title,
            handler_key,
            policy,
            bundles,
            js,
            css,
        });
    }

    Ok(pages)
}

fn registry_entry(
    raw_registry: &mlua::Table,
    index: usize,
) -> Result<Option<mlua::Table>, PluginError> {
    match raw_registry.get::<mlua::Value>(index).map_err(|e| {
        PluginError::InitFailed(format!(
            "invalid contract registry entry at index {index}: {e}"
        ))
    })? {
        mlua::Value::Table(entry) => Ok(Some(entry)),
        mlua::Value::Nil => Ok(None),
        _ => Err(PluginError::InitFailed(format!(
            "contract registry entry {index} must be a table"
        ))),
    }
}

fn required_string(entry: &mlua::Table, field: &str, index: usize) -> Result<String, PluginError> {
    entry.get::<String>(field).map_err(|e| {
        PluginError::InitFailed(format!(
            "contract registry admin entry {index} has invalid {field}: {e}"
        ))
    })
}

fn optional_string(
    entry: &mlua::Table,
    field: &str,
    index: usize,
) -> Result<Option<String>, PluginError> {
    match entry.get::<mlua::Value>(field).map_err(|e| {
        PluginError::InitFailed(format!(
            "contract registry admin entry {index} has invalid {field}: {e}"
        ))
    })? {
        mlua::Value::Nil => Ok(None),
        mlua::Value::String(value) => Ok(Some(
            value
                .to_str()
                .map_err(|e| {
                    PluginError::InitFailed(format!(
                        "contract registry admin entry {index} has invalid {field}: {e}"
                    ))
                })?
                .to_string(),
        )),
        _ => Err(PluginError::InitFailed(format!(
            "contract registry admin entry {index} field '{field}' must be a string"
        ))),
    }
}

fn resolve_handler_key(
    handlers: &mlua::Table,
    entry: &mlua::Table,
    index: usize,
) -> Result<String, PluginError> {
    let handler_key =
        optional_string(entry, "handler_key", index)?.filter(|value| !value.is_empty());
    match entry.get::<mlua::Value>("handler").map_err(|e| {
        PluginError::InitFailed(format!(
            "contract registry admin entry {index} has invalid handler: {e}"
        ))
    })? {
        mlua::Value::Nil => handler_key.ok_or_else(|| {
            PluginError::InitFailed(format!(
                "contract registry admin entry {index} must include handler or handler_key"
            ))
        }),
        mlua::Value::Function(handler) => {
            let key = handler_key.unwrap_or_else(|| {
                format!(
                    "contract_admin_handler_{}",
                    CONTRACT_ADMIN_HANDLER_COUNTER.fetch_add(1, Ordering::Relaxed)
                )
            });
            handlers.set(key.as_str(), handler).map_err(|e| {
                PluginError::InitFailed(format!(
                    "failed to cache admin handler for contract registry entry {index}: {e}"
                ))
            })?;
            Ok(key)
        }
        mlua::Value::String(value) => value.to_str().map(|value| value.to_string()).map_err(|e| {
            PluginError::InitFailed(format!(
                "contract registry admin entry {index} has invalid handler key: {e}"
            ))
        }),
        _ => Err(PluginError::InitFailed(format!(
            "contract registry admin entry {index} field 'handler' must be a function or string"
        ))),
    }
}

fn parse_assets(
    entry: &mlua::Table,
    index: usize,
) -> Result<(Vec<String>, Vec<String>, Vec<String>), PluginError> {
    let assets = match entry.get::<mlua::Value>("assets").map_err(|e| {
        PluginError::InitFailed(format!(
            "contract registry admin entry {index} has invalid assets: {e}"
        ))
    })? {
        mlua::Value::Nil => return Ok((Vec::new(), Vec::new(), Vec::new())),
        mlua::Value::Table(assets) => assets,
        _ => {
            return Err(PluginError::InitFailed(format!(
                "contract registry admin entry {index} field 'assets' must be a table"
            )))
        }
    };
    Ok((
        string_array(&assets, "bundles", index)?,
        string_array(&assets, "js", index)?,
        string_array(&assets, "css", index)?,
    ))
}

fn string_array(
    table: &mlua::Table,
    field: &str,
    index: usize,
) -> Result<Vec<String>, PluginError> {
    match table.get::<mlua::Value>(field).map_err(|e| {
        PluginError::InitFailed(format!(
            "contract registry admin entry {index} has invalid assets.{field}: {e}"
        ))
    })? {
        mlua::Value::Nil => Ok(Vec::new()),
        mlua::Value::Table(values) => {
            let len = values.raw_len();
            let mut entries = 0usize;
            for pair in values.clone().pairs::<mlua::Value, mlua::Value>() {
                let (key, _) = pair.map_err(|e| {
                    PluginError::InitFailed(format!(
                        "contract registry admin entry {index} has invalid assets.{field} keys: {e}"
                    ))
                })?;
                entries += 1;
                match key {
                    mlua::Value::Integer(array_index)
                        if array_index >= 1 && (array_index as usize) <= len => {}
                    _ => {
                        return Err(PluginError::InitFailed(format!(
                            "contract registry admin entry {index} assets.{field} must be an array of strings"
                        )))
                    }
                }
            }
            if entries != len {
                return Err(PluginError::InitFailed(format!(
                    "contract registry admin entry {index} assets.{field} must be an array of strings"
                )));
            }
            (1..=len).map(|array_index| {
                values.get::<String>(array_index).map_err(|e| {
                    PluginError::InitFailed(format!(
                        "contract registry admin entry {index} has invalid assets.{field}[{array_index}]: {e}"
                    ))
                })
            })
            .collect()
        }
        _ => Err(PluginError::InitFailed(format!(
            "contract registry admin entry {index} assets.{field} must be an array of strings"
        ))),
    }
}
