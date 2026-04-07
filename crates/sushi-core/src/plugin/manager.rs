use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Manages loaded Lua plugin VMs and dispatches handler calls.
#[derive(Clone, Default)]
pub struct PluginManager {
    vms: Arc<Mutex<HashMap<String, mlua::Lua>>>,
    api_handlers: Arc<Mutex<HashMap<(String, String), (String, String)>>>,
    cli_handlers: Arc<Mutex<HashMap<String, (String, String)>>>,
    admin_handlers: Arc<Mutex<HashMap<String, (String, String)>>>,
}

impl PluginManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Store a loaded Lua VM for a plugin.
    pub async fn register_vm(&self, plugin_name: &str, lua: mlua::Lua) {
        self.vms.lock().await.insert(plugin_name.to_string(), lua);
    }

    /// Register an API route handler.
    pub async fn register_api_handler(
        &self,
        method: &str,
        path: &str,
        plugin_name: &str,
        handler_key: &str,
    ) {
        self.api_handlers.lock().await.insert(
            (method.to_uppercase(), path.to_string()),
            (plugin_name.to_string(), handler_key.to_string()),
        );
    }

    /// Register a CLI command handler.
    pub async fn register_cli_handler(
        &self,
        command_name: &str,
        plugin_name: &str,
        handler_key: &str,
    ) {
        self.cli_handlers.lock().await.insert(
            command_name.to_string(),
            (plugin_name.to_string(), handler_key.to_string()),
        );
    }

    /// Register an admin page handler.
    pub async fn register_admin_handler(
        &self,
        page_path: &str,
        plugin_name: &str,
        handler_key: &str,
    ) {
        self.admin_handlers.lock().await.insert(
            page_path.to_string(),
            (plugin_name.to_string(), handler_key.to_string()),
        );
    }

    /// Call an API route handler, returns the response body as String.
    /// `body` is the raw request body (if any), passed to the Lua handler.
    /// Supports exact match and prefix wildcard (path ending with `*`).
    pub async fn call_api_handler(
        &self,
        method: &str,
        path: &str,
        body: Option<String>,
    ) -> Option<Result<String, String>> {
        let method_upper = method.to_uppercase();
        let map = self.api_handlers.lock().await;

        // Try exact match first
        let (plugin_name, handler_key) = if let Some(v) = map.get(&(method_upper.clone(), path.to_string())) {
            v.clone()
        } else {
            // Try prefix wildcard match: find entries where path ends with *
            let mut found = None;
            for ((m, prefix), val) in map.iter() {
                if m == &method_upper && prefix.ends_with('*') {
                    let prefix_str = &prefix[..prefix.len() - 1];
                    if path.starts_with(prefix_str) {
                        found = Some(val.clone());
                        break;
                    }
                }
            }
            found?
        };
        drop(map);

        // Pass path + body as args so Lua handler can extract path params
        let args = match body {
            Some(b) => vec![path.to_string(), b],
            None => vec![path.to_string()],
        };
        Some(self.call_handler_with_args(&plugin_name, &handler_key, &args).await)
    }

    /// Call a CLI command handler with string args, returns the output.
    pub async fn call_cli_handler(
        &self,
        command_name: &str,
        args: &[String],
    ) -> Option<Result<String, String>> {
        let (plugin_name, handler_key) = {
            let map = self.cli_handlers.lock().await;
            map.get(command_name).cloned()?
        };
        Some(self.call_handler_with_args(&plugin_name, &handler_key, args).await)
    }

    /// Call an admin page handler, returns HTML String.
    pub async fn call_admin_handler(&self, page_path: &str) -> Option<Result<String, String>> {
        let (plugin_name, handler_key) = {
            let map = self.admin_handlers.lock().await;
            map.get(page_path).cloned()?
        };
        Some(self.call_handler_no_args(&plugin_name, &handler_key).await)
    }

    /// List all registered API routes.
    pub async fn list_api_routes(&self) -> Vec<(String, String)> {
        self.api_handlers
            .lock()
            .await
            .keys()
            .map(|(m, p)| (m.clone(), p.clone()))
            .collect()
    }

    /// List all registered CLI commands.
    pub async fn list_cli_commands(&self) -> Vec<String> {
        self.cli_handlers.lock().await.keys().cloned().collect()
    }

    /// List all registered admin pages.
    pub async fn list_admin_pages(&self) -> Vec<String> {
        self.admin_handlers.lock().await.keys().cloned().collect()
    }

    // -- private helpers --

    async fn call_handler_no_args(
        &self,
        plugin_name: &str,
        handler_key: &str,
    ) -> Result<String, String> {
        let vms = self.vms.lock().await;
        let lua = vms
            .get(plugin_name)
            .ok_or_else(|| format!("plugin '{plugin_name}' not loaded"))?;

        let func = self.get_handler_fn(lua, handler_key)?;
        func.call_async::<String>(())
            .await
            .map_err(|e| format!("handler error: {e}"))
    }

    async fn call_handler_with_args(
        &self,
        plugin_name: &str,
        handler_key: &str,
        args: &[String],
    ) -> Result<String, String> {
        let vms = self.vms.lock().await;
        let lua = vms
            .get(plugin_name)
            .ok_or_else(|| format!("plugin '{plugin_name}' not loaded"))?;

        let func = self.get_handler_fn(lua, handler_key)?;

        // Build a Lua table from args
        let args_table = lua.create_table()
            .map_err(|e| format!("create args table: {e}"))?;
        for (i, arg) in args.iter().enumerate() {
            args_table.set(i + 1, arg.clone())
                .map_err(|e| format!("set arg: {e}"))?;
        }

        func.call_async::<String>(args_table)
            .await
            .map_err(|e| format!("handler error: {e}"))
    }

    fn get_handler_fn(
        &self,
        lua: &mlua::Lua,
        handler_key: &str,
    ) -> Result<mlua::Function, String> {
        let sushi: mlua::Table = lua
            .globals()
            .get("sushi")
            .map_err(|e| format!("no sushi global: {e}"))?;

        let handlers: mlua::Table = sushi
            .get("__handlers")
            .map_err(|e| format!("no __handlers table: {e}"))?;

        handlers
            .get(handler_key)
            .map_err(|e| format!("handler '{handler_key}' not found: {e}"))
    }
}
