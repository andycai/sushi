use crate::plugin::PluginError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventEntry {
    pub kind: String,
    pub event: String,
    pub handler_key: Option<String>,
}

pub fn snapshot_from_lua(raw_registry: mlua::Table) -> Result<Vec<EventEntry>, PluginError> {
    let mut entries = Vec::new();
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
        if surface != "event" {
            continue;
        }

        let kind = entry.get::<String>("kind").map_err(|e| {
            PluginError::InitFailed(format!(
                "contract registry event entry {index} has invalid kind: {e}"
            ))
        })?;
        let event = entry.get::<String>("event").map_err(|e| {
            PluginError::InitFailed(format!(
                "contract registry event entry {index} has invalid event: {e}"
            ))
        })?;
        let handler_key = match entry.get::<mlua::Value>("handler_key").map_err(|e| {
            PluginError::InitFailed(format!(
                "contract registry event entry {index} has invalid handler_key: {e}"
            ))
        })? {
            mlua::Value::Nil => None,
            mlua::Value::String(value) => Some(
                value
                    .to_str()
                    .map_err(|e| {
                        PluginError::InitFailed(format!(
                            "contract registry event entry {index} has invalid handler_key: {e}"
                        ))
                    })?
                    .to_string(),
            ),
            _ => {
                return Err(PluginError::InitFailed(format!(
                    "contract registry event entry {index} handler_key must be a string"
                )))
            }
        };

        entries.push(EventEntry {
            kind,
            event,
            handler_key,
        });
    }

    Ok(entries)
}
