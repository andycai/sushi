# sushi.db

数据库接口，用于执行 SQL 查询和操作。

## 可用性

- **需要权限** - 需要在 `plugin.toml` 中配置 `database` 权限

## 权限级别

| 配置值 | 说明 |
|-------|------|
| `false` | 无数据库访问 |
| `true` / `"read"` | 只读查询 |
| `"write"` | 读写（INSERT/UPDATE/DELETE） |
| `"admin"` | 完全访问（包括 DROP/ALTER） |

## 方法

### `sushi.db.query(sql, params?)`

执行 SQL 查询并返回结果行。

**参数：**
- `sql` (string): SQL 查询语句
- `params` (table, optional): 查询参数数组

**返回值：**
- (table): 结果行数组，每行是一个 table

**示例：**
```lua
-- 简单查询
local rows = sushi.db.query("SELECT * FROM users")
for _, row in ipairs(rows) do
    print(row.id, row.username)
end

-- 带参数查询
local rows = sushi.db.query(
    "SELECT * FROM users WHERE role = ?1",
    { "admin" }
)

-- 带多个参数
local rows = sushi.db.query(
    "SELECT * FROM items WHERE name = ?1 AND active = ?2",
    { "test", true }
)
```

---

### `sushi.db.execute(sql, params?)`

执行 SQL 语句（INSERT/UPDATE/DELETE/CREATE 等）。

**参数：**
- `sql` (string): SQL 语句
- `params` (table, optional): 执行参数数组

**返回值：**
- 无

**异常：**
- 只读权限调用时抛出异常

**示例：**
```lua
-- 创建表
sushi.db.execute([[
    CREATE TABLE IF NOT EXISTS items (
        id INTEGER PRIMARY KEY,
        name TEXT NOT NULL,
        created_at TEXT DEFAULT CURRENT_TIMESTAMP
    )
]])

-- 插入数据
sushi.db.execute(
    "INSERT INTO items (name) VALUES (?1)",
    { "New Item" }
)

-- 更新数据
sushi.db.execute(
    "UPDATE items SET name = ?1 WHERE id = ?2",
    { "Updated", 42 }
)

-- 删除数据
sushi.db.execute("DELETE FROM items WHERE id = ?1", { 42 })
```

---

## 参数绑定

使用 `?1`, `?2` 等占位符，参数从数组中按顺序取值。

**支持的参数类型：**
- `string` → TEXT
- `number` (整数/浮点) → INTEGER/REAL
- `boolean` → INTEGER (0 或 1)
- `nil` → NULL

---

## 安全限制

- 查询有超时机制，防止阻塞主线程
- DROP/ALTER 需要 `admin` 权限
- 不支持多语句执行（防止 SQL 注入）

---

## 错误处理

```lua
local ok, err = pcall(function()
    sushi.db.execute("INVALID SQL")
end)

if not ok then
    sushi.log.error("DB error: " .. tostring(err))
end
```
