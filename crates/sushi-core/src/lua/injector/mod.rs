use crate::lua::permission::engine::{CapabilityKind, PermissionDecisionEngine};
use crate::plugin::Permissions;
use mlua::Lua;

/// Inject contract-kernel entrypoints into an existing `sushi` table.
pub fn inject(lua: &Lua, permissions: Permissions, enabled: bool) -> Result<(), mlua::Error> {
    let sushi: mlua::Table = lua.globals().get("sushi")?;
    let decision_engine = PermissionDecisionEngine::new(permissions, enabled);

    let capability = lua.create_table()?;
    capability.set(
        "register",
        lua.create_function(|lua, entry: mlua::Table| {
            let sushi: mlua::Table = lua.globals().get("sushi")?;
            let pending: mlua::Table = sushi.get("__contract_registry")?;
            let len = pending.raw_len();
            pending.set(len + 1, entry)?;
            Ok(())
        })?,
    )?;
    sushi.set("capability", capability)?;

    if decision_engine.is_visible(CapabilityKind::ApiRoute) {
        sushi.set("api", lua.create_table()?)?;
    }

    Ok(())
}
