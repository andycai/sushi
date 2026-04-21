use crate::plugin::PluginError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FsEntry {
    pub kind: String,
    pub root: String,
}

pub fn snapshot_from_lua(raw_registry: mlua::Table) -> Result<Vec<FsEntry>, PluginError> {
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
        if surface != "fs" {
            continue;
        }

        let kind = entry.get::<String>("kind").map_err(|e| {
            PluginError::InitFailed(format!(
                "contract registry fs entry {index} has invalid kind: {e}"
            ))
        })?;
        let root = entry.get::<String>("root").map_err(|e| {
            PluginError::InitFailed(format!(
                "contract registry fs entry {index} has invalid root: {e}"
            ))
        })?;

        entries.push(FsEntry { kind, root });
    }

    Ok(entries)
}
