use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_LUA_RUNTIME_ID: AtomicU64 = AtomicU64::new(1);

pub struct LuaRuntimeInstance {
    id: u64,
    plugin_name: String,
    lua: mlua::Lua,
}

impl LuaRuntimeInstance {
    pub fn new(plugin_name: impl Into<String>, lua: mlua::Lua) -> Self {
        Self {
            id: NEXT_LUA_RUNTIME_ID.fetch_add(1, Ordering::Relaxed),
            plugin_name: plugin_name.into(),
            lua,
        }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn plugin_name(&self) -> &str {
        &self.plugin_name
    }

    pub fn lua(&self) -> &mlua::Lua {
        &self.lua
    }
}

impl fmt::Debug for LuaRuntimeInstance {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LuaRuntimeInstance")
            .field("id", &self.id)
            .field("plugin_name", &self.plugin_name)
            .finish_non_exhaustive()
    }
}

impl PartialEq for LuaRuntimeInstance {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for LuaRuntimeInstance {}
