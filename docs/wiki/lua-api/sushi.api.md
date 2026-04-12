# sushi.api

HTTP 路由接口，用于注册 API 端点。

## 可用性

- **需要权限** - 需要在 `plugin.toml` 中配置 `routes = true`

## 方法

### `sushi.api.route(method, path, handler)`

注册一个 HTTP 路由。

**参数：**
- `method` (string): HTTP 方法，如 `"GET"`, `"POST"`, `"PUT"`, `"DELETE"`, `"PATCH"`
- `path` (string): 路由路径，如 `"/api/items"`
- `handler` (function): 处理函数

**处理函数签名：**
```lua
function handler(request) --> response
    -- request: 包含以下字段的 table
    --   method: string
    --   path: string
    --   headers: table
    --   body: any (解析后的请求体)
    --
    -- 返回值: response table
    --   status: number (HTTP 状态码)
    --   body: any (响应体)
end
```

**示例：**
```lua
-- GET 路由
sushi.api.route("GET", "/api/items", function(req)
    local rows = sushi.db.query("SELECT * FROM items")
    return {
        status = 200,
        body = rows
    }
end)

-- POST 路由
sushi.api.route("POST", "/api/items", function(req)
    sushi.db.execute(
        "INSERT INTO items (name) VALUES (?1)",
        { req.body.name }
    )
    return {
        status = 201,
        body = { message = "Created" }
    }
end)

-- 获取请求头
sushi.api.route("GET", "/api/profile", function(req)
    local auth_header = req.headers["authorization"]
    local claims = sushi.auth.verify_token(auth_header)
    if not claims then
        return { status = 401, body = { error = "Unauthorized" } }
    end
    return { status = 200, body = claims }
end)
```

---

## 内部机制

注册的路由存储在 `sushi.__pending_routes` 表中，由框架在插件初始化后统一注册到 Axum 路由器。

**内部表结构：**
```lua
{
    {
        method = "GET",
        path = "/api/items",
        handler_key = "h_1"
    },
    ...
}
```

---

## 响应格式

| 字段 | 类型 | 说明 |
|-----|------|------|
| `status` | number | HTTP 状态码（200, 201, 400, 401, 404, 500 等） |
| `body` | any | 响应体，会自动序列化为 JSON |

**标准响应示例：**
```lua
-- 成功
return { status = 200, body = { data = "value" } }

-- 创建成功
return { status = 201, body = { id = 1 } }

-- 错误
return { status = 400, body = { error = "Invalid input" } }
```
