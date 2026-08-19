use crate::lua::contract::schema::cli::CliCommandContract;
use crate::plugin::PluginError;
use std::sync::atomic::{AtomicU64, Ordering};

static CONTRACT_CLI_HANDLER_COUNTER: AtomicU64 = AtomicU64::new(1);

pub fn snapshot_from_lua(
    lua: &mlua::Lua,
    raw_registry: mlua::Table,
) -> Result<Vec<CliCommandContract>, PluginError> {
    let app: mlua::Table = lua
        .globals()
        .get("app")
        .map_err(|e| PluginError::InitFailed(format!("missing app global: {e}")))?;
    let handlers: mlua::Table = app
        .get("__handlers")
        .map_err(|e| PluginError::InitFailed(format!("missing app.__handlers table: {e}")))?;
    let mut commands = Vec::new();

    for index in 1..=raw_registry.raw_len() {
        let value = raw_registry.get::<mlua::Value>(index).map_err(|e| {
            PluginError::InitFailed(format!(
                "invalid contract registry entry at index {index}: {e}"
            ))
        })?;
        let entry = match value {
            mlua::Value::Table(entry) => entry,
            mlua::Value::Nil => continue,
            _ => {
                return Err(PluginError::InitFailed(format!(
                    "contract registry entry {index} must be a table"
                )))
            }
        };
        if entry.get::<Option<String>>("surface").map_err(|e| {
            PluginError::InitFailed(format!(
                "contract registry entry {index} has invalid surface: {e}"
            ))
        })? != Some("cli".to_string())
        {
            continue;
        }

        let name = entry.get::<String>("name").map_err(|e| {
            PluginError::InitFailed(format!(
                "contract registry cli entry {index} has invalid name: {e}"
            ))
        })?;
        let description = entry.get::<String>("description").map_err(|e| {
            PluginError::InitFailed(format!(
                "contract registry cli entry {index} has invalid description: {e}"
            ))
        })?;
        let policy = optional_string(&entry, "policy", index)?;
        let handler_key = resolve_handler_key(&handlers, &entry, index)?;
        commands.push(CliCommandContract {
            name,
            description,
            handler_key,
            policy,
        });
    }

    Ok(commands)
}

fn optional_string(
    entry: &mlua::Table,
    field: &str,
    index: usize,
) -> Result<Option<String>, PluginError> {
    match entry.get::<mlua::Value>(field).map_err(|e| {
        PluginError::InitFailed(format!(
            "contract registry cli entry {index} has invalid {field}: {e}"
        ))
    })? {
        mlua::Value::Nil => Ok(None),
        mlua::Value::String(value) => Ok(Some(
            value
                .to_str()
                .map_err(|e| {
                    PluginError::InitFailed(format!(
                        "contract registry cli entry {index} has invalid {field}: {e}"
                    ))
                })?
                .to_string(),
        )),
        _ => Err(PluginError::InitFailed(format!(
            "contract registry cli entry {index} field '{field}' must be a string"
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
            "contract registry cli entry {index} has invalid handler: {e}"
        ))
    })? {
        mlua::Value::Nil => handler_key.ok_or_else(|| {
            PluginError::InitFailed(format!(
                "contract registry cli entry {index} must include handler or handler_key"
            ))
        }),
        mlua::Value::Function(handler) => {
            let key = handler_key.unwrap_or_else(|| {
                format!(
                    "contract_cli_handler_{}",
                    CONTRACT_CLI_HANDLER_COUNTER.fetch_add(1, Ordering::Relaxed)
                )
            });
            handlers.set(key.as_str(), handler).map_err(|e| {
                PluginError::InitFailed(format!(
                    "failed to cache cli handler for contract registry entry {index}: {e}"
                ))
            })?;
            Ok(key)
        }
        mlua::Value::String(value) => value.to_str().map(|value| value.to_string()).map_err(|e| {
            PluginError::InitFailed(format!(
                "contract registry cli entry {index} has invalid handler key: {e}"
            ))
        }),
        _ => Err(PluginError::InitFailed(format!(
            "contract registry cli entry {index} field 'handler' must be a function or string"
        ))),
    }
}
