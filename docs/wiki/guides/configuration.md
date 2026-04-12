# 配置指南

## 配置文件

Sushi 使用 TOML 格式配置文件：`config.toml`

## 完整配置示例

```toml
[server]
host = "127.0.0.1"
port = 3000
body_size_limit = 65536  # 64KB

[database]
path = "data/sushi.db"

[jwt]
secret = "your-secret-key-at-least-32-characters-long"
access_ttl = 3600        # 1 hour in seconds
refresh_ttl = 604800      # 7 days in seconds

[plugins]
directory = "plugins"

[web]
templates_dir = "web/templates"
static_dir = "web/static"
static_url_prefix = "/static"
```

## 配置项说明

### server

| 配置项 | 类型 | 默认值 | 说明 |
|-------|------|-------|------|
| `host` | string | `"127.0.0.1"` | 监听地址 |
| `port` | number | `3000` | 监听端口 |
| `body_size_limit` | number | `65536` | 请求体大小限制（字节） |

### database

| 配置项 | 类型 | 默认值 | 说明 |
|-------|------|-------|------|
| `path` | string | `"data/sushi.db"` | SQLite 数据库路径 |

### jwt

| 配置项 | 类型 | 默认值 | 说明 |
|-------|------|-------|------|
| `secret` | string | 自动生成 | JWT 签名密钥（生产环境必须设置） |
| `access_ttl` | number | `3600` | Access token 有效期（秒） |
| `refresh_ttl` | number | `604800` | Refresh token 有效期（秒） |

### plugins

| 配置项 | 类型 | 默认值 | 说明 |
|-------|------|-------|------|
| `directory` | string | `"plugins"` | 插件目录路径 |

### web

| 配置项 | 类型 | 默认值 | 说明 |
|-------|------|-------|------|
| `templates_dir` | string | `"web/templates"` | 模板目录 |
| `static_dir` | string | `"web/static"` | 静态文件目录 |
| `static_url_prefix` | string | `"/static"` | 静态资源 URL 前缀 |

## 启动命令

```bash
# 启动完整服务（API + Admin）
sushi serve

# 只启动 API
sushi serve --api

# 只启动 Admin
sushi serve --admin

# 运行插件
sushi run <plugin-name>

# CLI 帮助
sushi --help
```
