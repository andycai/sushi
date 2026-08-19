# Profile 组合指南

Sushi 使用有序 profile/bundle 描述一次运行实例需要哪些宿主能力和插件。产品装配不再依赖散落在 bootstrap 中的目录扫描与条件分支。

## 文件布局

```text
profiles/
├── default.toml
├── api.toml
├── admin.toml
└── minimal.toml

bundles/
├── base.toml
├── api.toml
├── admin.toml
└── official.toml
```

profile 和 bundle 都使用 schema v1。文件名必须与文档中的 `name` 相同。

## Bundle 条目

```toml
schema_version = 1
name = "official"

[[entries]]
id = "cms.default"
source = "lua:official/cms"
enabled = true
required = false

[entries.config]
editor_mode = "standard"

[entries.grants]
database = "admin"
```

每个条目包含：

- `id`：稳定的插件实例 ID，也是 capability owner；治理、卸载和诊断都使用该 ID。
- `source`：`builtin:<key>` 或 `lua:<tier>/<directory>`。
- `enabled`：是否允许该条目参与启动；`false` 会覆盖已有可选插件治理状态并跳过激活。
- `required`：是否为系统必需条目；required 必须同时 `enabled = true`，普通 Admin/CLI toggle 会被拒绝。
- `config`：插件私有配置，当前作为不透明 JSON/TOML 值保留给后续 RuntimePlugin contract。
- `grants`：profile 授权配置。`database = "write" | "admin"` 已作为插件 migration 的显式门禁；普通运行时能力仍按 trust/grant 路线继续收敛。

Lua source 必须是 `official/<name>` 或 `third_party/<name>` 两级相对路径，并在插件目录中存在 `plugin.toml`。绝对路径、`..`、未知 builtin 和重复 Lua source 都会在启动前失败。

## Profile 与 Overlay

```toml
schema_version = 1
name = "default"
bundles = ["base", "api", "admin", "official"]

[[overlays]]
id = "cms.default"
source = "lua:official/cms"
enabled = true
required = false

[overlays.config]
editor_mode = "compact"
```

组合顺序固定为：

1. 按 `bundles` 声明顺序追加完整条目。
2. 按 `overlays` 声明顺序替换已有稳定 ID。
3. 输出保持 bundle 插入顺序；overlay 不改变条目位置。

Overlay 是**完整条目替换**，不是 deep merge。示例中的 overlay 不会保留 bundle 原有 `config` 字段；需要保留的字段必须重新声明。未知 overlay target、重复 ID 或重复 overlay 都会失败。

## 内置 Profile

| Profile | Identity | API Core | Host Admin | RBAC Admin | CLI 基线 | 官方 Lua |
|---------|----------|----------|------------|------------|----------|----------|
| `default` | 是 | 是 | 是 | 是 | 是 | CMS、File Browser、KV Store |
| `api` | 是 | 是 | 否 | 否 | 是 | CMS、File Browser、KV Store |
| `admin` | 否 | 否 | 是 | 是 | 是 | CMS、File Browser、KV Store |
| `minimal` | 否 | 否 | 否 | 否 | 是 | 无 |

所有 `serve` profile 都保留稳定 `/health`。required `identity` 独占 login、refresh、me 认证 API，required `api-core` 独占 users API；`host-admin` 注册通用 Admin 能力，required `rbac-admin` 独占 users、roles、permissions 页面、表格 partial、CRUD 和角色权限分配。Host Router 仅保留稳定 transport 边界和尚未迁移的系统路由。

## Profile 与数据库迁移

启动流程先解析 profile，再打开数据库并汇总被选择条目的 migration catalog，最后才激活插件。

- 平台历史 migration 使用稳定 owner 和 migration ID 写入 `plugin_migrations`。
- 官方 Lua 插件的 SQL 必须位于 `plugins/official/<name>/migrations/*.sql`，文件名以数字顺序开头，例如 `010_add_index.sql`。
- Lua migration 同时要求官方 source、manifest 的数据库写权限，以及 profile 的显式 `grants.database = "write" | "admin"`。
- 未被 profile 选择或被 `enabled = false` 的插件不会执行其 migration。
- `minimal` 不执行 Admin 菜单和官方业务插件 migration；`api` 不执行 Admin 菜单 migration。
- migration 只向前执行；禁用插件或回滚 profile 不自动回滚数据库结构。
- 已存在的 `_sushi_migrations` 历史记录会桥接到新 catalog，不重复执行历史 SQL；checksum 不一致会拒绝启动。

## 兼容发现

未显式设置 `[runtime].profile` 时：

1. 若 `profiles/default.toml` 存在，严格加载 `default`。
2. 若默认 profile 文件不存在，使用内部 `legacy-default`：注入完整 Host 基线，并按路径排序发现 `plugins/official/*` 与 `plugins/third_party/*`。

显式 profile 解析失败时不会回退到全目录扫描，也不会创建数据库或激活插件。

## 检查命令

```bash
# 只解析配置和组合，不打开数据库
sushi inspect profile --config config.toml --profile default

# 完成 bootstrap 后输出 capability key 与 owner
sushi inspect capabilities --config config.toml --profile default
```

`inspect profile` 输出稳定 JSON，包含最终顺序、source、enabled/required、config/grants 和最后来源。`inspect capabilities` 的 capability map 按 key 排序，不输出运行期自增 registration ID。
