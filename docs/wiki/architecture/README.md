# 架构总览

Sushi 是一个通用应用平台，采用模块化架构。

## 系统架构图

```
┌─────────────────────────────────────────────────────────────┐
│                      sushi (binary)                        │
├─────────────┬─────────────┬─────────────┬─────────────────┤
│  sushi-cli │  sushi-api  │ sushi-admin │    plugins/     │
│             │             │             │                 │
│  clap CLI   │  Axum HTTP  │  Axum + UI  │  Lua Plugins    │
└──────┬──────┴──────┬──────┴──────┬──────┴────────┬────────┘
       │             │             │               │
       └─────────────┴──────┬──────┴───────────────┘
                            │
                    ┌───────┴───────┐
                    │  sushi-core  │
                    ├───────────────┤
                    │ Plugin Trait │
                    │ Lua VM       │
                    │ Auth/JWT     │
                    │ RBAC         │
                    │ EventBus     │
                    │ Storage      │
                    │ Config       │
                    └───────┬───────┘
                            │
                    ┌───────┴───────┐
                    │   SQLite     │
                    │  (rusqlite)  │
                    └──────────────┘
```

## 核心模块

| 模块 | Crate | 说明 |
|-----|-------|------|
| 插件系统 | `sushi-core/src/plugin/` | Plugin trait, manifest, permissions |
| Lua 运行时 | `sushi-core/src/lua/` | mlua 集成, bindings, VM |
| 认证授权 | `sushi-core/src/auth/` | JWT, Password, RBAC |
| 数据库 | `sushi-core/src/db/` | DbGateway, migrations |
| 存储 | `sushi-core/src/storage/` | Storage trait, SQLite 实现 |
| 配置 | `sushi-core/src/config/` | SushiConfig, ConfigStore |
| Web | `sushi-core/src/web/` | 模板服务 |
| 事件总线 | `sushi-core/src/registry/` | EventBus |

## Crate 依赖关系

```
sushi/
├── sushi-cli/    → sushi-core
├── sushi-api/    → sushi-core
├── sushi-admin/  → sushi-core, sushi-api
└── sushi/        → sushi-core, sushi-api, sushi-admin, sushi-cli
```
