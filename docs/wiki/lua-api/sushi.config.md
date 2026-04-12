# sushi.config

配置接口。

## 可用性

- **始终可用** - 无需任何权限配置

## 方法

### `sushi.config.get(key)`

获取配置值（当前为 stub 实现，总返回 nil）。

**参数：**
- `key` (string): 配置键名

**返回值：**
- (nil): 当前实现总返回 nil

**示例：**
```lua
local value = sushi.config.get("server.port")
-- 当前返回 nil（stub）
```

---

## 实现状态

> 注意：此接口为只读 stub。配置管理功能开发中。
