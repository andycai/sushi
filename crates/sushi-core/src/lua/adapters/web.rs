use crate::plugin::PluginError;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebPageEntry {
    pub path: String,
    pub title: String,
    pub handler_key: String,
    pub policy: Option<String>,
    pub bundle_names: Vec<String>,
    pub page_js: Vec<String>,
    pub page_css: Vec<String>,
}

static CONTRACT_WEB_HANDLER_COUNTER: AtomicU64 = AtomicU64::new(1);

pub fn snapshot_from_lua(
    lua: &mlua::Lua,
    raw_registry: mlua::Table,
) -> Result<Vec<WebPageEntry>, PluginError> {
    let sushi: mlua::Table = lua
        .globals()
        .get("sushi")
        .map_err(|e| PluginError::InitFailed(format!("missing sushi global: {e}")))?;
    let handlers: mlua::Table = sushi
        .get("__handlers")
        .map_err(|e| PluginError::InitFailed(format!("missing sushi.__handlers table: {e}")))?;

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
        if surface != "web" {
            continue;
        }

        let kind = entry.get::<String>("kind").map_err(|e| {
            PluginError::InitFailed(format!(
                "contract registry web entry {index} has invalid kind: {e}"
            ))
        })?;
        if kind != "page" {
            continue;
        }

        let path = entry.get::<String>("path").map_err(|e| {
            PluginError::InitFailed(format!(
                "contract registry web entry {index} has invalid path: {e}"
            ))
        })?;
        let template = parse_optional_string(&entry, "template", index, "web")?;
        let title = parse_optional_string(&entry, "title", index, "web")?
            .unwrap_or_else(|| template.unwrap_or_else(|| path.clone()));
        let policy = parse_optional_string(&entry, "policy", index, "web")?;
        let (bundle_names, page_js, page_css) = parse_assets(&entry, index)?;

        let handler_from_key = parse_optional_string(&entry, "handler_key", index, "web")?
            .filter(|value| !value.is_empty());
        let handler_key = resolve_handler_key(&handlers, &entry, handler_from_key, index)?;

        pages.push(WebPageEntry {
            path,
            title,
            handler_key,
            policy,
            bundle_names,
            page_js,
            page_css,
        });
    }

    Ok(pages)
}

fn parse_assets(
    entry: &mlua::Table,
    index: usize,
) -> Result<(Vec<String>, Vec<String>, Vec<String>), PluginError> {
    let assets = match entry.get::<mlua::Value>("assets").map_err(|e| {
        PluginError::InitFailed(format!(
            "contract registry web entry {index} has invalid assets: {e}"
        ))
    })? {
        mlua::Value::Nil => return Ok((Vec::new(), Vec::new(), Vec::new())),
        mlua::Value::Table(table) => table,
        _ => {
            return Err(PluginError::InitFailed(format!(
                "contract registry web entry {index} field 'assets' must be a table"
            )))
        }
    };

    let bundles = parse_optional_string_array(&assets, "bundles", index)?;
    let js = parse_optional_string_array(&assets, "js", index)?;
    let css = parse_optional_string_array(&assets, "css", index)?;
    Ok((bundles, js, css))
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

fn parse_optional_string_array(
    entry: &mlua::Table,
    field: &str,
    index: usize,
) -> Result<Vec<String>, PluginError> {
    match entry.get::<mlua::Value>(field).map_err(|e| {
        PluginError::InitFailed(format!(
            "contract registry web entry {index} has invalid assets.{field}: {e}"
        ))
    })? {
        mlua::Value::Nil => Ok(Vec::new()),
        mlua::Value::Table(values) => {
            let len = values.raw_len();
            let mut entries = 0usize;
            for pair in values.pairs::<mlua::Value, mlua::Value>() {
                let (key, _) = pair.map_err(|e| {
                    PluginError::InitFailed(format!(
                        "contract registry web entry {index} has invalid assets.{field} keys: {e}"
                    ))
                })?;
                entries += 1;
                match key {
                    mlua::Value::Integer(array_index)
                        if array_index >= 1 && (array_index as usize) <= len => {}
                    _ => {
                        return Err(PluginError::InitFailed(format!(
                            "contract registry web entry {index} assets.{field} must be an array of strings"
                        )))
                    }
                }
            }
            if entries != len {
                return Err(PluginError::InitFailed(format!(
                    "contract registry web entry {index} assets.{field} must be an array of strings"
                )));
            }

            let mut out = Vec::with_capacity(len);
            for array_index in 1..=len {
                let value = values.get::<mlua::Value>(array_index).map_err(|e| {
                    PluginError::InitFailed(format!(
                        "contract registry web entry {index} has invalid assets.{field}[{array_index}] value: {e}"
                    ))
                })?;
                let item = match value {
                    mlua::Value::String(item) => item
                        .to_str()
                        .map_err(|e| {
                            PluginError::InitFailed(format!(
                                "contract registry web entry {index} has invalid utf-8 in assets.{field}[{array_index}]: {e}"
                            ))
                        })?
                        .to_string(),
                    _ => {
                        return Err(PluginError::InitFailed(format!(
                            "contract registry web entry {index} assets.{field} entries must be strings"
                        )))
                    }
                };
                out.push(item);
            }
            Ok(out)
        }
        _ => Err(PluginError::InitFailed(format!(
            "contract registry web entry {index} assets.{field} must be an array of strings"
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
            "contract registry web entry {index} has invalid handler: {e}"
        ))
    })? {
        mlua::Value::Nil => handler_key.ok_or_else(|| {
            PluginError::InitFailed(format!(
                "contract registry web entry {index} must include handler or handler_key"
            ))
        }),
        mlua::Value::Function(handler_fn) => {
            let resolved_key = handler_key
                .unwrap_or_else(|| format!("contract_web_handler_{}", next_handler_id()));
            handlers
                .set(resolved_key.as_str(), handler_fn)
                .map_err(|e| {
                    PluginError::InitFailed(format!(
                        "failed to cache web handler for contract registry entry {index}: {e}"
                    ))
                })?;
            Ok(resolved_key)
        }
        mlua::Value::String(value) => Ok(value
            .to_str()
            .map_err(|e| {
                PluginError::InitFailed(format!(
                    "contract registry web entry {index} has invalid handler key: {e}"
                ))
            })?
            .to_string()),
        _ => Err(PluginError::InitFailed(format!(
            "contract registry web entry {index} field 'handler' must be a function or string"
        ))),
    }
}

fn next_handler_id() -> u64 {
    CONTRACT_WEB_HANDLER_COUNTER.fetch_add(1, Ordering::Relaxed)
}
