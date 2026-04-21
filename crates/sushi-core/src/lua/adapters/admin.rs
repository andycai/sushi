use crate::lua::contract::schema::admin::AdminPageContract;
use crate::plugin::PluginError;

pub fn snapshot_from_lua(raw_registry: mlua::Table) -> Result<Vec<AdminPageContract>, PluginError> {
    let mut pages = Vec::new();
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

        if surface != "admin" {
            continue;
        }

        let path = entry.get::<String>("path").map_err(|e| {
            PluginError::InitFailed(format!(
                "contract registry admin entry {index} has invalid path: {e}"
            ))
        })?;
        let title = entry.get::<String>("title").map_err(|e| {
            PluginError::InitFailed(format!(
                "contract registry admin entry {index} has invalid title: {e}"
            ))
        })?;

        pages.push(AdminPageContract { path, title });
    }

    Ok(pages)
}
