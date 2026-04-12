# sushi.web

Web 渲染接口，提供模板渲染和 JSON 响应功能。

## 可用性

- **需要权限** - 需要 `admin = true` 或 `routes = true`

| 方法 | 权限要求 |
|-----|---------|
| `render` | `admin = true` 或 `routes = true` |
| `page` | `admin = true` |
| `json` | `admin = true` 或 `routes = true` |

## 方法

### `sushi.web.render(template_name, context?)`

渲染一个模板文件并返回 HTML。

**参数：**
- `template_name` (string): 模板路径，如 `"admin/login.html"`
- `context` (table, optional): 模板上下文数据

**返回值：**
- (string): 渲染后的 HTML

**示例：**
```lua
local html = sushi.web.render("admin/login.html", {
    title = "Login",
    error = nil
})

-- 在路由中使用
sushi.api.route("GET", "/page", function(req)
    local html = sushi.web.render("admin/page.html", {
        title = "Dynamic Page",
        items = { "a", "b", "c" }
    })
    return { status = 200, body = html }
end)
```

---

### `sushi.web.page(path, template_name, opts)`

注册一个 Admin 页面（`sushi.admin.page` 的替代写法）。

**参数：**
- `path` (string): URL 路径
- `template_name` (string): 模板路径
- `opts` (table, optional): 选项
  - `title` (string): 页面标题
  - `context` (table): 模板上下文

**示例：**
```lua
sushi.web.page("/admin/report", "admin/report.html", {
    title = "Sales Report",
    context = {
        report_date = os.date("%Y-%m-%d")
    }
})
```

**与 `sushi.admin.page` 的区别：**

| 特性 | `sushi.web.page` | `sushi.admin.page` |
|-----|------------------|-------------------|
| 上下文 | 模板文件 + context table | 直接返回 HTML 字符串 |
| 模板引擎 | 继承框架模板系统 | 自定义渲染逻辑 |

---

### `sushi.web.json(status, data)`

生成 JSON 响应（用于 API 路由）。

**参数：**
- `status` (number): HTTP 状态码
- `data` (any): 响应数据

**返回值：**
- (string): JSON 字符串（包含 `__sushi_web_json` envelope）

**示例：**
```lua
sushi.api.route("GET", "/api/status", function(req)
    return {
        status = 200,
        body = sushi.web.json(200, { status = "ok", uptime = 3600 })
    }
end)

-- 实际返回格式：
-- {
--   "__sushi_web_json": true,
--   "status": 200,
--   "body": { "status": "ok", "uptime": 3600 }
-- }
```

---

## 模板变量

所有模板默认注入以下变量：

| 变量 | 说明 |
|-----|------|
| `static_url_prefix` | 静态资源 URL 前缀 |

**模板示例 (admin/login.html)：**
```html
<!DOCTYPE html>
<html>
<head>
    <link rel="stylesheet" href="{{ static_url_prefix }}/css/styles.css">
</head>
<body>
    <h1>{{ title }}</h1>
</body>
</html>
```

---

## 安全

`static_url_prefix` 是只读的，Lua 代码无法覆盖：
```lua
-- 这不会生效
sushi.web.render("test.html", { static_url_prefix = "/evil" })
-- 实际渲染时仍使用配置的值
```
