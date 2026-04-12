# 插件开发指南

## 创建插件

### 1. 创建插件目录

```
plugins/
└── my_plugin/
    ├── plugin.toml
    └── init.lua
```

### 2. 编写 plugin.toml

```toml
[plugin]
name = "my_plugin"
version = "0.1.0"
description = "My first plugin"
entry = "init.lua"

[permissions]
routes = true
commands = true
admin = true
database = "read"
```

### 3. 编写 init.lua

```lua
function sushi.init()
    -- 注册路由
    sushi.api.route("GET", "/api/my-plugin/items", function(req)
        local rows = sushi.db.query("SELECT * FROM items")
        return { status = 200, body = rows }
    end)

    -- 注册 CLI 命令
    sushi.cli.command("items:count", "Count all items", function(args)
        local rows = sushi.db.query("SELECT COUNT(*) as count FROM items")
        print("Total items: " .. rows[1].count)
    end)

    -- 注册 Admin 页面
    sushi.admin.page("/admin/my-plugin", "My Plugin", function()
        return [[
            <div class="p-4">
                <h1>My Plugin</h1>
                <p>Welcome to my plugin!</p>
            </div>
        ]]
    end)

    sushi.log.info("my_plugin initialized")
end
```

## 插件示例

### 带数据库读写的插件

```toml
[permissions]
routes = true
database = "write"
```

```lua
function sushi.init()
    -- 初始化数据库表
    sushi.db.execute([[
        CREATE TABLE IF NOT EXISTS todos (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            done INTEGER DEFAULT 0,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP
        )
    ]])

    -- 注册 API 路由
    sushi.api.route("GET", "/api/todos", function(req)
        local rows = sushi.db.query("SELECT * FROM todos ORDER BY id DESC")
        return { status = 200, body = rows }
    end)

    sushi.api.route("POST", "/api/todos", function(req)
        local title = req.body.title
        if not title or title == "" then
            return { status = 400, body = { error = "Title required" } }
        end
        sushi.db.execute("INSERT INTO todos (title) VALUES (?1)", { title })
        return { status = 201, body = { message = "Created" } }
    end)

    sushi.api.route("PUT", "/api/todos/:id", function(req)
        local id = req.body.id
        local done = req.body.done and 1 or 0
        sushi.db.execute("UPDATE todos SET done = ?1 WHERE id = ?2", { done, id })
        return { status = 200, body = { message = "Updated" } }
    end)

    sushi.api.route("DELETE", "/api/todos/:id", function(req)
        local id = req.body.id
        sushi.db.execute("DELETE FROM todos WHERE id = ?1", { id })
        return { status = 200, body = { message = "Deleted" } }
    end)

    -- Admin 页面
    sushi.admin.page("/admin/todos", "Todos", function()
        return [[
            <div x-data="todosApp()" class="p-4">
                <h1 class="text-2xl font-bold mb-4">Todos</h1>
                <form @submit.prevent="addTodo()" class="mb-4">
                    <input x-model="newTitle" type="text" placeholder="New todo..."
                           class="border p-2 rounded mr-2">
                    <button type="submit" class="bg-blue-500 text-white px-4 py-2 rounded">
                        Add
                    </button>
                </form>
                <ul>
                    <template x-for="todo in todos" :key="todo.id">
                        <li class="flex items-center gap-2 mb-2">
                            <input type="checkbox" :checked="todo.done == 1"
                                   @change="toggleTodo(todo)">
                            <span x-text="todo.title"></span>
                        </li>
                    </template>
                </ul>
            </div>
            <script>
                function todosApp() {
                    return {
                        todos: [],
                        newTitle: '',
                        async addTodo() {
                            let res = await fetch('/api/todos', {
                                method: 'POST',
                                headers: { 'Content-Type': 'application/json' },
                                body: JSON.stringify({ title: this.newTitle })
                            });
                            if (res.ok) {
                                this.newTitle = '';
                                this.loadTodos();
                            }
                        },
                        async toggleTodo(todo) {
                            await fetch('/api/todos', {
                                method: 'PUT',
                                headers: { 'Content-Type': 'application/json' },
                                body: JSON.stringify({ id: todo.id, done: todo.done == 1 ? 0 : 1 })
                            });
                            this.loadTodos();
                        },
                        async loadTodos() {
                            let res = await fetch('/api/todos');
                            this.todos = await res.json();
                        }
                    }
                }
            </script>
        ]]
    end)

    sushi.log.info("todos plugin initialized")
end
```

## 最佳实践

1. **错误处理** - 使用 `pcall` 包装可能出错的代码
2. **日志记录** - 使用 `sushi.log` 记录关键操作
3. **参数验证** - API 入口验证输入参数
4. **权限最小化** - 只申请需要的权限
5. **避免阻塞** - 数据库操作使用异步接口

## 调试

```lua
-- 添加日志调试
sushi.log.info("Debug: " .. inspect(table))
```

## 相关文档

- [Lua API 参考](../lua-api/README.md)
- [插件系统](../architecture/plugin-system.md)
