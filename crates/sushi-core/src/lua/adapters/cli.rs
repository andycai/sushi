use crate::lua::contract::schema::cli::CliCommandContract;
use crate::plugin::PluginError;

pub fn snapshot_from_lua(
    raw_registry: mlua::Table,
) -> Result<Vec<CliCommandContract>, PluginError> {
    let mut commands = Vec::new();
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

        if surface != "cli" {
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

        commands.push(CliCommandContract { name, description });
    }

    Ok(commands)
}
