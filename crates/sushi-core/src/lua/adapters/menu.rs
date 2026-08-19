use crate::lua::contract::schema::menu::MenuContributionContract;
use crate::plugin::PluginError;

pub fn snapshot_from_lua(
    raw_registry: mlua::Table,
) -> Result<Vec<MenuContributionContract>, PluginError> {
    let mut contributions = Vec::new();
    for index in 1..=raw_registry.raw_len() {
        let value = raw_registry.get::<mlua::Value>(index).map_err(|error| {
            PluginError::InitFailed(format!(
                "invalid contract registry entry at index {index}: {error}"
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
        if entry.get::<Option<String>>("surface").map_err(|error| {
            PluginError::InitFailed(format!(
                "contract registry entry {index} has invalid surface: {error}"
            ))
        })? != Some("menu".to_string())
        {
            continue;
        }

        let id = required_string(&entry, "id", index)?;
        let label = required_string(&entry, "label", index)?;
        let position = entry
            .get::<Option<i64>>("position")
            .map_err(|error| {
                PluginError::InitFailed(format!(
                    "contract registry menu entry {index} has invalid position: {error}"
                ))
            })?
            .unwrap_or_default();
        contributions.push(MenuContributionContract {
            id,
            label,
            icon: optional_string(&entry, "icon", index)?,
            position,
            parent_id: optional_string(&entry, "parent_id", index)?,
            route: optional_string(&entry, "route", index)?,
            policy: optional_string(&entry, "policy", index)?,
        });
    }
    Ok(contributions)
}

fn required_string(entry: &mlua::Table, field: &str, index: usize) -> Result<String, PluginError> {
    entry.get::<String>(field).map_err(|error| {
        PluginError::InitFailed(format!(
            "contract registry menu entry {index} has invalid {field}: {error}"
        ))
    })
}

fn optional_string(
    entry: &mlua::Table,
    field: &str,
    index: usize,
) -> Result<Option<String>, PluginError> {
    match entry.get::<mlua::Value>(field).map_err(|error| {
        PluginError::InitFailed(format!(
            "contract registry menu entry {index} has invalid {field}: {error}"
        ))
    })? {
        mlua::Value::Nil => Ok(None),
        mlua::Value::String(value) => Ok(Some(
            value
                .to_str()
                .map_err(|error| {
                    PluginError::InitFailed(format!(
                        "contract registry menu entry {index} has invalid {field}: {error}"
                    ))
                })?
                .to_string(),
        )),
        _ => Err(PluginError::InitFailed(format!(
            "contract registry menu entry {index} field '{field}' must be a string"
        ))),
    }
}
