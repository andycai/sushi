# sushi.log

日志命名空间，提供结构化日志功能。

## 可用性

- **始终可用** - 无需任何权限配置

## 方法

### `sushi.log.info(msg)`

记录 info 级别日志。

**参数：**
- `msg` (string): 日志消息

**示例：**
```lua
sushi.log.info("Plugin initialized successfully")
sushi.log.info("User " .. username .. " logged in")
```

---

### `sushi.log.warn(msg)`

记录 warn 级别日志。

**参数：**
- `msg` (string): 日志消息

**示例：**
```lua
sushi.log.warn("Deprecated API called: " .. endpoint)
sushi.log.warn("Config value missing, using default")
```

---

### `sushi.log.error(msg)`

记录 error 级别日志。

**参数：**
- `msg` (string): 日志消息

**示例：**
```lua
sushi.log.error("Database connection failed")
sushi.log.error("Unexpected error: " .. err)
```

---

## 实现细节

日志通过 `tracing` crate 输出，格式为 `[lua] <message>`。
