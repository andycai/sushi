use crate::plugin::PluginError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbEntry {
    pub kind: String,
    pub name: String,
}

pub fn snapshot_from_lua(raw_registry: mlua::Table) -> Result<Vec<DbEntry>, PluginError> {
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
        if surface != "db" {
            continue;
        }

        let kind = entry.get::<String>("kind").map_err(|e| {
            PluginError::InitFailed(format!(
                "contract registry db entry {index} has invalid kind: {e}"
            ))
        })?;
        let name = entry.get::<String>("name").map_err(|e| {
            PluginError::InitFailed(format!(
                "contract registry db entry {index} has invalid name: {e}"
            ))
        })?;

        entries.push(DbEntry { kind, name });
    }

    Ok(entries)
}
