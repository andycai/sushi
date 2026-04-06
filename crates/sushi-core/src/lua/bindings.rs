use std::sync::Arc;

use crate::context::SushiContext;
use crate::kv::KvStore;
use crate::plugin::Permissions;
use mlua::Lua;

/// Inject the `sushi` global table into the Lua VM.
/// Only namespaces permitted by the plugin's permissions are injected.
pub async fn inject_sushi_api(
    lua: &Lua,
    ctx: &SushiContext,
    permissions: &Permissions,
) -> Result<(), mlua::Error> {
    let sushi = lua.create_table()?;

    // sushi.log -- always available
    {
        let log_table = lua.create_table()?;
        log_table.set(
            "info",
            lua.create_function(|_, msg: String| {
                tracing::info!("[lua] {msg}");
                Ok(())
            })?,
        )?;
        log_table.set(
            "warn",
            lua.create_function(|_, msg: String| {
                tracing::warn!("[lua] {msg}");
                Ok(())
            })?,
        )?;
        log_table.set(
            "error",
            lua.create_function(|_, msg: String| {
                tracing::error!("[lua] {msg}");
                Ok(())
            })?,
        )?;
        sushi.set("log", log_table)?;
    }

    // sushi.api -- if routes permitted
    if permissions.routes {
        let pending_routes = lua.create_table()?;
        sushi.set("__pending_routes", pending_routes)?;

        let api_table = lua.create_table()?;
        api_table.set(
            "route",
            lua.create_function(
                move |lua, (method, path, handler): (String, String, mlua::Function)| {
                    let pending: mlua::Table = lua
                        .globals()
                        .get::<mlua::Table>("sushi")?
                        .get("__pending_routes")?;
                    let entry = lua.create_table()?;
                    entry.set("method", method)?;
                    entry.set("path", path)?;
                    entry.set("handler", handler)?;
                    let len = pending.raw_len();
                    pending.set(len + 1, entry)?;
                    Ok(())
                },
            )?,
        )?;
        sushi.set("api", api_table)?;
    }

    // sushi.cli -- if commands permitted
    if permissions.commands {
        let pending_commands = lua.create_table()?;
        sushi.set("__pending_commands", pending_commands)?;

        let cli_table = lua.create_table()?;
        cli_table.set(
            "command",
            lua.create_function(move |lua, (name, desc): (String, String)| {
                let pending: mlua::Table = lua
                    .globals()
                    .get::<mlua::Table>("sushi")?
                    .get("__pending_commands")?;
                let entry = lua.create_table()?;
                entry.set("name", name)?;
                entry.set("description", desc)?;
                let len = pending.raw_len();
                pending.set(len + 1, entry)?;
                Ok(())
            })?,
        )?;
        sushi.set("cli", cli_table)?;
    }

    // sushi.admin -- if admin permitted
    if permissions.admin {
        let pending_pages = lua.create_table()?;
        sushi.set("__pending_pages", pending_pages)?;

        let admin_table = lua.create_table()?;
        admin_table.set(
            "page",
            lua.create_function(move |lua, (path, title): (String, String)| {
                let pending: mlua::Table = lua
                    .globals()
                    .get::<mlua::Table>("sushi")?
                    .get("__pending_pages")?;
                let entry = lua.create_table()?;
                entry.set("path", path)?;
                entry.set("title", title)?;
                let len = pending.raw_len();
                pending.set(len + 1, entry)?;
                Ok(())
            })?,
        )?;
        sushi.set("admin", admin_table)?;
    }

    // sushi.kv -- if database permitted (not None)
    if permissions.database != crate::plugin::DatabasePermission::None {
        let kv_store = KvStore::new(ctx.db.clone());
        let kv_table = lua.create_table()?;

        let kv_store_get = kv_store.clone();
        kv_table.set(
            "get",
            lua.create_async_function(move |lua: Lua, key: String| {
                let kv = kv_store_get.clone();
                async move {
                    match kv.get(&key).await {
                        Ok(Some(value)) => Ok(mlua::Value::String(lua.create_string(&value).unwrap())),
                        Ok(None) => Ok(mlua::Value::Nil),
                        Err(e) => Err(mlua::Error::ExternalError(Arc::new(e) as Arc<dyn std::error::Error + Send + Sync>)),
                    }
                }
            })?,
        )?;

        let kv_store_set = kv_store.clone();
        kv_table.set(
            "set",
            lua.create_async_function(move |_lua: Lua, (key, value): (String, String)| {
                let kv = kv_store_set.clone();
                async move {
                    kv.set(&key, &value)
                        .await
                        .map_err(|e| mlua::Error::ExternalError(Arc::new(e) as Arc<dyn std::error::Error + Send + Sync>))
                }
            })?,
        )?;

        let kv_store_del = kv_store.clone();
        kv_table.set(
            "delete",
            lua.create_async_function(move |_lua: Lua, key: String| {
                let kv = kv_store_del.clone();
                async move {
                    kv.delete(&key)
                        .await
                        .map_err(|e| mlua::Error::ExternalError(Arc::new(e) as Arc<dyn std::error::Error + Send + Sync>))
                }
            })?,
        )?;

        let kv_store_list = kv_store.clone();
        kv_table.set(
            "list",
            lua.create_async_function(move |lua: Lua, ()| {
                let kv = kv_store_list.clone();
                async move {
                    match kv.list().await {
                        Ok(items) => {
                            let table = lua.create_table()?;
                            for (i, (k, v)) in items.into_iter().enumerate() {
                                let entry = lua.create_table()?;
                                entry.set("key", k)?;
                                entry.set("value", v)?;
                                table.set(i + 1, entry)?;
                            }
                            Ok(mlua::Value::Table(table))
                        }
                        Err(e) => Err(mlua::Error::ExternalError(Arc::new(e) as Arc<dyn std::error::Error + Send + Sync>)),
                    }
                }
            })?,
        )?;

        sushi.set("kv", kv_table)?;
    }

    // sushi.config -- always available (read-only stub)
    {
        let config_table = lua.create_table()?;
        config_table.set(
            "get",
            lua.create_function(|_, _key: String| Ok(mlua::Value::Nil))?,
        )?;
        sushi.set("config", config_table)?;
    }

    // sushi.event -- always available
    {
        let event_table = lua.create_table()?;
        event_table.set(
            "on",
            lua.create_function(|_, (_event, _callback): (String, mlua::Function)| Ok(()))?,
        )?;
        event_table.set(
            "emit",
            lua.create_function(|_, (_event, _data): (String, mlua::Value)| Ok(()))?,
        )?;
        sushi.set("event", event_table)?;
    }

    // sushi.auth -- always available
    {
        let auth_table = lua.create_table()?;
        auth_table.set(
            "verify_token",
            lua.create_function(|_, _token: String| Ok(mlua::Value::Nil))?,
        )?;
        sushi.set("auth", auth_table)?;
    }

    lua.globals().set("sushi", sushi)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::jwt::JwtService;
    use crate::config::ConfigStore;
    use crate::lua::vm::create_sandboxed_vm;
    use crate::plugin::Permissions;
    use crate::storage::sqlite::SqliteStorage;

    /// Build a minimal SushiContext for testing bindings.
    async fn test_context() -> SushiContext {
        let config = ConfigStore::new(crate::config::SushiConfig::default());
        let db = SqliteStorage::new_in_memory().await.unwrap();
        let jwt = JwtService::new("test-secret-key-at-least-32-chars-long!", 3600, 604800);
        SushiContext::new(config, db, jwt)
    }

    #[tokio::test]
    async fn test_inject_always_available_namespaces() {
        let lua = create_sandboxed_vm().unwrap();
        let ctx = test_context().await;
        let permissions = Permissions::default();

        inject_sushi_api(&lua, &ctx, &permissions).await.unwrap();

        let sushi: mlua::Table = lua.globals().get("sushi").unwrap();

        // Always-available namespaces
        assert!(sushi.contains_key("log").unwrap());
        assert!(sushi.contains_key("config").unwrap());
        assert!(sushi.contains_key("event").unwrap());
        assert!(sushi.contains_key("auth").unwrap());
    }

    #[tokio::test]
    async fn test_inject_routes_permission() {
        let lua = create_sandboxed_vm().unwrap();
        let ctx = test_context().await;
        let mut permissions = Permissions::default();
        permissions.routes = true;

        inject_sushi_api(&lua, &ctx, &permissions).await.unwrap();

        let sushi: mlua::Table = lua.globals().get("sushi").unwrap();
        assert!(sushi.contains_key("api").unwrap());
        assert!(sushi.contains_key("__pending_routes").unwrap());
    }

    #[tokio::test]
    async fn test_inject_commands_permission() {
        let lua = create_sandboxed_vm().unwrap();
        let ctx = test_context().await;
        let mut permissions = Permissions::default();
        permissions.commands = true;

        inject_sushi_api(&lua, &ctx, &permissions).await.unwrap();

        let sushi: mlua::Table = lua.globals().get("sushi").unwrap();
        assert!(sushi.contains_key("cli").unwrap());
        assert!(sushi.contains_key("__pending_commands").unwrap());
    }

    #[tokio::test]
    async fn test_inject_admin_permission() {
        let lua = create_sandboxed_vm().unwrap();
        let ctx = test_context().await;
        let mut permissions = Permissions::default();
        permissions.admin = true;

        inject_sushi_api(&lua, &ctx, &permissions).await.unwrap();

        let sushi: mlua::Table = lua.globals().get("sushi").unwrap();
        assert!(sushi.contains_key("admin").unwrap());
        assert!(sushi.contains_key("__pending_pages").unwrap());
    }

    #[tokio::test]
    async fn test_no_api_without_routes_permission() {
        let lua = create_sandboxed_vm().unwrap();
        let ctx = test_context().await;
        let permissions = Permissions::default();

        inject_sushi_api(&lua, &ctx, &permissions).await.unwrap();

        let sushi: mlua::Table = lua.globals().get("sushi").unwrap();
        assert!(!sushi.contains_key("api").unwrap());
        assert!(!sushi.contains_key("cli").unwrap());
        assert!(!sushi.contains_key("admin").unwrap());
    }

    #[tokio::test]
    async fn test_log_functions_callable() {
        let lua = create_sandboxed_vm().unwrap();
        let ctx = test_context().await;
        let permissions = Permissions::default();

        inject_sushi_api(&lua, &ctx, &permissions).await.unwrap();

        // Should not error — logging functions just trace
        lua.load("sushi.log.info('hello from lua')").exec().unwrap();
        lua.load("sushi.log.warn('warning')").exec().unwrap();
        lua.load("sushi.log.error('error')").exec().unwrap();
    }

    #[tokio::test]
    async fn test_api_route_registration() {
        let lua = create_sandboxed_vm().unwrap();
        let ctx = test_context().await;
        let mut permissions = Permissions::default();
        permissions.routes = true;

        inject_sushi_api(&lua, &ctx, &permissions).await.unwrap();

        lua.load("sushi.api.route('GET', '/api/test', function() end)")
            .exec()
            .unwrap();
        lua.load("sushi.api.route('POST', '/api/items', function() end)")
            .exec()
            .unwrap();

        let pending: mlua::Table = lua
            .globals()
            .get::<mlua::Table>("sushi")
            .unwrap()
            .get("__pending_routes")
            .unwrap();
        assert_eq!(pending.raw_len(), 2);

        let first: mlua::Table = pending.get(1).unwrap();
        let method: String = first.get("method").unwrap();
        let path: String = first.get("path").unwrap();
        assert_eq!(method, "GET");
        assert_eq!(path, "/api/test");

        // Verify handler is stored as a function
        let _handler: mlua::Function = first.get("handler").unwrap();
    }
}
