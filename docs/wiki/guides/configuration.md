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

[file_browser]
root_dir = "."

[web]
templates_dir = "web/templates"
static_dir = "web/static"
static_url_prefix = "/static"

[runtime]
profile = "default"
profiles_dir = "profiles"
bundles_dir = "bundles"
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

### file_browser

| 配置项 | 类型 | 默认值 | 说明 |
|-------|------|-------|------|
| `root_dir` | string | `"."` | 文件浏览器根目录基准路径（`plugin.toml` 中相对 `path` 会相对该目录解析） |

### runtime

| 配置项 | 类型 | 默认值 | 说明 |
|-------|------|-------|------|
| `profile` | string / null | `null` | 运行时 profile；未配置时严格加载 `default`，默认文件不存在则 fail closed |
| `profiles_dir` | string | `"profiles"` | profile 文件目录，相对配置文件所在目录解析 |
| `bundles_dir` | string | `"bundles"` | bundle 文件目录，相对配置文件所在目录解析 |

## 启动命令

```bash
# 启动完整服务（API + Admin）
sushi serve

# 只启动 API
sushi serve --profile api

# 只启动 Admin
sushi serve --profile admin

# 最小恢复模式，仅保留 /health 与 bootstrap-safe CLI
sushi serve --profile minimal

# 不打开数据库，检查最终 profile
sushi inspect profile --profile default

# 启动所选插件后检查 owner-scoped capability
sushi inspect capabilities --profile default

# CLI 帮助
sushi --help
```

`serve` 只通过全局 `--profile` 选择产品组合。进程收到 Ctrl-C，或 Unix 上收到 SIGTERM 时，会停止接收新连接并等待在途请求完成后退出。

## CLI 配置管理

`config get/set` 使用全局 `--config` 指定的文件和点号键：

```bash
# 读取当前 runtime 快照中的生效值，包括默认补全
sushi --config config.toml config get server.port
sushi --config config.toml config get runtime.profile

# 只修改目标 TOML 字段，保留注释和其他 section
sushi --config config.toml config set server.port 4100
sushi --config config.toml config set runtime.profile minimal

# 删除可选 profile 值，恢复 default profile 解析
sushi --config config.toml config set runtime.profile null
```

首版支持本页“配置项说明”中的叶子字段。未知键、错误类型、越界数值或缺失配置文件都会失败且不修改文件。`jwt.secret` 不能通过 CLI 读取或设置，避免敏感值进入终端、日志、shell history 或进程参数。

`set` 成功后只影响下一次启动；当前进程不会热重载配置。写入使用同目录临时文件原子替换，保留 TOML 注释、未知字段和已有文件权限。

Profile 与 bundle 的完整语义见 [Profile 组合指南](profile-composition.md)。
