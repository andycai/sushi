# 4. Architecture & Design Patterns (架构规范与设计约束)

*核心设计模式，防止破坏项目架构*

- **CLI 敏感配置：** 允许通过 `config set jwt.secret <VALUE>` 更新 secret 会把值暴露给 shell history 和进程参数 -> CLI 点号键读写必须排除 secret，敏感值使用受保护文件编辑或专用秘密管理入口。
- **Reload effect 代际：** reload 前后复用同一个 owner 时，按 owner 全量撤销会误杀新 effect，不撤销会泄漏旧 effect -> `PluginHandle` 必须保存 registration IDs，成功发布新 generation 后按上一代 IDs 定向撤销；失败 reload 保留旧 generation。
- **Profile schema owner：** 自定义测试 profile 未选择 `host-core/policy` 却依赖治理与 RBAC 表，migration factory 化后在 policy refresh 提前失败 -> 需要平台 schema 的 profile 必须显式选择对应 builtin owner，测试 fixture 不得依赖中央隐式 baseline migration。

<!-- 容量上限：15-20 条。超出时合并或归档旧条目 -->
