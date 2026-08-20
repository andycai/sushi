use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use super::{
    PluginCancellationToken, PluginInstanceId, PluginLifecycleState, RegistrationId,
    TaskRegistration,
};

static NEXT_LUA_RUNTIME_ID: AtomicU64 = AtomicU64::new(1);

pub struct LuaRuntimeInstance {
    id: u64,
    plugin_name: String,
    lua: mlua::Lua,
}

#[derive(Clone)]
pub struct PluginHandle {
    pub owner: PluginInstanceId,
    pub runtime: Arc<LuaRuntimeInstance>,
    pub state: PluginLifecycleState,
    pub registrations: Vec<RegistrationId>,
    pub tasks: Vec<TaskRegistration>,
    pub cancellation: PluginCancellationToken,
}

impl PluginHandle {
    pub fn new(
        owner: PluginInstanceId,
        runtime: Arc<LuaRuntimeInstance>,
        state: PluginLifecycleState,
        registrations: Vec<RegistrationId>,
        tasks: Vec<TaskRegistration>,
        cancellation: PluginCancellationToken,
    ) -> Self {
        Self {
            owner,
            runtime,
            state,
            registrations,
            tasks,
            cancellation,
        }
    }

    pub fn with_state(&self, state: PluginLifecycleState) -> Self {
        Self {
            owner: self.owner.clone(),
            runtime: Arc::clone(&self.runtime),
            state,
            registrations: self.registrations.clone(),
            tasks: self.tasks.clone(),
            cancellation: self.cancellation.clone(),
        }
    }
}

impl fmt::Debug for PluginHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PluginHandle")
            .field("owner", &self.owner)
            .field("runtime", &self.runtime)
            .field("state", &self.state)
            .field("registrations", &self.registrations)
            .field("tasks", &self.tasks)
            .field("cancelled", &self.cancellation.is_cancelled())
            .finish()
    }
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
