# sushi.cli

CLI 命令接口，用于注册命令行子命令。

## 可用性

- **需要权限** - 需要在 `plugin.toml` 中配置 `commands = true`

## 方法

### `sushi.cli.command(name, description, handler)`

注册一个 CLI 子命令。

**参数：**
- `name` (string): 命令名称，如 `"items:list"`, `"greet"`
- `description` (string): 命令描述（用于 `--help` 输出）
- `handler` (function): 处理函数

**处理函数签名：**
```lua
function handler(args) --> void
    -- args: 命令行参数 table
    --   _[1], _[2], ...: 位置参数
    --   其他字段: 选项（如果有）
end
```

**示例：**
```lua
-- 简单命令
sushi.cli.command("greet", "Print a greeting", function(args)
    print("Hello, World!")
end)

-- 带参数命令
sushi.cli.command("items:list", "List all items", function(args)
    local rows = sushi.db.query("SELECT * FROM items")
    for _, row in ipairs(rows) do
        print(row.id .. " | " .. row.name)
    end
end)

-- 解析选项
sushi.cli.command("items:filter", "Filter items", function(args)
    local filter = args.filter or "all"
    local limit = args.limit or 100

    local query = "SELECT * FROM items"
    if filter ~= "all" then
        query = query .. " WHERE category = '" .. filter .. "'"
    end
    query = query .. " LIMIT " .. limit

    local rows = sushi.db.query(query)
    for _, row in ipairs(rows) do
        print(row.id .. " | " .. row.name)
    end
end)
```

---

## 调用方式

注册后，命令通过以下方式调用：

```bash
# 插件注册的命令
sushi items:list
sushi items:filter --filter=electronics --limit=50
sushi greet

# 通过 run 子命令运行单个插件
sushi run <plugin-name>
```

---

## 内部机制

注册的命令存储在 `sushi.__pending_commands` 表中。

**内部表结构：**
```lua
{
    {
        name = "items:list",
        description = "List all items",
        handler_key = "h_1"
    },
    ...
}
```
