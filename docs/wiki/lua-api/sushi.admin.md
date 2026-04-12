# sushi.admin

Admin 页面接口，用于扩展管理面板。

## 可用性

- **需要权限** - 需要在 `plugin.toml` 中配置 `admin = true`

## 方法

### `sushi.admin.page(path, title, handler)`

注册一个 Admin 页面。

**参数：**
- `path` (string): URL 路径，如 `"/admin/my-plugin"`
- `title` (string): 页面标题
- `handler` (function): 渲染函数，返回 HTML 内容

**处理函数签名：**
```lua
function handler() --> string
    -- 返回 HTML 字符串
end
```

**示例：**
```lua
sushi.admin.page("/admin/my-plugin", "My Plugin", function()
    return [[
        <div x-data="{ items: [], loading: true }" x-init="
            fetch('/api/my-plugin/items')
                .then(r => r.json())
                .then(d => { items = d; loading = false })
        ">
            <h1>My Plugin Items</h1>
            <div x-show="loading">Loading...</div>
            <table x-show="!loading" class="min-w-full">
                <thead>
                    <tr>
                        <th>ID</th>
                        <th>Name</th>
                    </tr>
                </thead>
                <tbody>
                    <template x-for="item in items" :key="item.id">
                        <tr>
                            <td x-text="item.id"></td>
                            <td x-text="item.name"></td>
                        </tr>
                    </template>
                </tbody>
            </table>
        </div>
    ]]
end)
```

---

## 页面渲染上下文

页面函数返回 HTML 后，框架会自动注入以下上下文变量：

| 变量 | 说明 |
|-----|------|
| `static_url_prefix` | 静态资源 URL 前缀（如 `/static`） |

**模板中使用：**
```html
<link rel="stylesheet" href="{{ static_url_prefix }}/css/styles.css">
```

---

## 内部机制

注册的页面存储在 `sushi.__pending_pages` 表中。

**内部表结构：**
```lua
{
    {
        path = "/admin/my-plugin",
        title = "My Plugin",
        handler_key = "h_1"
    },
    ...
}
```

---

## 页面与路由的区别

| 特性 | `sushi.admin.page` | `sushi.api.route` |
|-----|---------------------|-------------------|
| 渲染方式 | 返回 HTML 字符串 | 返回 JSON |
| 内容类型 | text/html | application/json |
| 适用场景 | 管理界面 | API 调用 |
| 权限 | 需要 `admin = true` | 需要 `routes = true` |
