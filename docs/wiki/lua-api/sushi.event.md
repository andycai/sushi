# sushi.event

事件总线接口，用于插件间通信。

## 可用性

- **始终可用** - 无需任何权限配置

## 方法

### `sushi.event.on(event, callback)`

注册事件监听器。

**参数：**
- `event` (string): 事件名称
- `callback` (function): 回调函数，接收 `data` 参数

**示例：**
```lua
sushi.event.on("user.created", function(data)
    sushi.log.info("New user: " .. data.username)
end)

sushi.event.on("plugin.loaded", function(data)
    sushi.log.info("Plugin loaded: " .. data.name)
end)
```

---

### `sushi.event.emit(event, data)`

发射事件到事件总线。

**参数：**
- `event` (string): 事件名称
- `data` (any): 事件数据（Lua 值，会被序列化为 JSON）

**示例：**
```lua
sushi.event.emit("item.created", { id = 42, name = "New Item" })

sushi.event.emit("my-plugin.data-changed", {
    item_id = 42,
    action = "update",
    timestamp = os.time()
})
```

---

## 内置事件

| 事件名 | 触发时机 | 数据 |
|--------|---------|------|
| `plugin.loaded` | 插件加载完成 | `{ name, version }` |
| `plugin.unloaded` | 插件卸载 | `{ name }` |
| `server.starting` | 服务启动前 | `{ port, mode }` |
| `server.started` | 服务启动后 | `{ port, mode }` |
| `user.created` | 用户创建 | `{ user_id, username }` |
| `user.login` | 用户登录 | `{ user_id, ip }` |

---

## 实现状态

> 注意：当前事件发射（emit）为异步操作，但完整的事件监听（on）需要架构调整才能完全支持。
