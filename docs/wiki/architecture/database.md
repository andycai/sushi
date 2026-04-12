# 数据库层

## 存储抽象

```rust
pub trait Storage: Send + Sync {
    async fn execute(&self, query: &str, params: Vec<Value>) -> Result<(), StorageError>;
    async fn query(&self, query: &str, params: Vec<Value>) -> Result<Vec<Row>, StorageError>;
}
```

## SQLite 实现

- 使用 `rusqlite` 库
- 通过 tokio task 隔离阻塞 IO
- 存储路径：`data/sushi.db`（可配置）

## 数据库迁移

内置迁移创建以下表：

| 表名 | 说明 |
|-----|------|
| `users` | 用户表 |
| `roles` | 角色表 |
| `permissions` | 权限表 |
| `role_permissions` | 角色-权限关联表 |
| `_sushi_migrations` | 迁移记录表 |

## DbGateway

数据库网关，提供带权限控制的数据库访问：

```rust
pub enum DbPermission {
    ReadOnly,   // 只读
    Write,      // 读写
    Admin,      // 完全访问
}
```

### Lua 侧使用

```lua
-- 查询（需要 ReadOnly 或更高权限）
local rows = sushi.db.query("SELECT * FROM users")

-- 执行（需要 Write 或更高权限）
sushi.db.execute("INSERT INTO users (name) VALUES (?1)", { "test" })
```

## 安全限制

| 操作 | 需要权限 |
|-----|---------|
| SELECT | ReadOnly |
| INSERT/UPDATE/DELETE | Write |
| CREATE/DROP/ALTER | Admin |

## 事务支持

Lua API 暂不支持事务（规划中）。
