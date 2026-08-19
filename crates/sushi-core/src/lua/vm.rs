use mlua::Lua;

/// Create a new sandboxed Lua 5.4 VM.
/// Dangerous globals (os.execute, io, etc.) are removed.
pub fn create_sandboxed_vm() -> Result<Lua, mlua::Error> {
    let lua = Lua::new();

    let globals = lua.globals();

    // Nullify dangerous os functions
    let os_table: mlua::Table = globals.get("os")?;
    os_table.set("execute", mlua::Value::Nil)?;
    os_table.set("exit", mlua::Value::Nil)?;
    os_table.set("getenv", mlua::Value::Nil)?;
    os_table.set("remove", mlua::Value::Nil)?;
    os_table.set("rename", mlua::Value::Nil)?;
    os_table.set("tmpname", mlua::Value::Nil)?;

    // Remove dangerous libraries and functions
    globals.set("io", mlua::Value::Nil)?;
    globals.set("package", mlua::Value::Nil)?;
    globals.set("require", mlua::Value::Nil)?;
    globals.set("dofile", mlua::Value::Nil)?;
    globals.set("loadfile", mlua::Value::Nil)?;

    // await(x) — identity function, no-op for async compatibility
    // Plugins can write `local result = await(app.db.query(...))` and it
    // works identically in both Sushi (async) and Suxun (sync).
    globals.set(
        "await",
        lua.create_function(|_, args: mlua::MultiValue| Ok(args))?,
    )?;

    Ok(lua)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_blocks_os_execute() {
        let lua = create_sandboxed_vm().unwrap();
        let result = lua.load("os.execute('echo test')").exec();
        assert!(result.is_err());
    }

    #[test]
    fn test_sandbox_blocks_io() {
        let lua = create_sandboxed_vm().unwrap();
        let result = lua.load("io.open('test.txt')").exec();
        assert!(result.is_err());
    }

    #[test]
    fn test_sandbox_blocks_require() {
        let lua = create_sandboxed_vm().unwrap();
        let result = lua.load("require('os')").exec();
        assert!(result.is_err());
    }

    #[test]
    fn test_sandbox_allows_basic_lua() {
        let lua = create_sandboxed_vm().unwrap();
        let result: i32 = lua.load("1 + 2").eval().unwrap();
        assert_eq!(result, 3);
    }

    #[test]
    fn test_sandbox_allows_tables() {
        let lua = create_sandboxed_vm().unwrap();
        let result: i32 = lua
            .load("local t = {a = 1, b = 2}; return t.a + t.b")
            .eval()
            .unwrap();
        assert_eq!(result, 3);
    }

    #[test]
    fn test_sandbox_allows_string_ops() {
        let lua = create_sandboxed_vm().unwrap();
        let result: String = lua.load("string.upper('hello')").eval().unwrap();
        assert_eq!(result, "HELLO");
    }
}
