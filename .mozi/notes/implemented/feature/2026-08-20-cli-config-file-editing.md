# Agent Note: 限制 CLI 配置文件编辑边界

Status: implemented

## 问题

`sushi config get/set` 长期只输出占位文本。直接把它实现为任意 TOML 路径编辑器，会让拼写错误静默写入未知字段、整体序列化丢失用户注释，或把 `jwt.secret` 暴露到终端、日志、shell history 和进程参数。配置写入还必须避免在校验失败时破坏原文件，并明确当前进程不会热重载新值。

## 决策

CLI 配置命令只接受 `SushiConfig` 已知叶子字段的点号键。`config get` 从本次 runtime 的 `ConfigStore` 快照读取，因此会展示 Serde 默认补全后的生效值；字符串使用 TOML 字面量，数值直接输出，未设置的 `runtime.profile` 输出 `null`。

`config set` 使用动态 launcher 实际解析的 `--config` 路径，要求目标文件已经存在。实现通过 `toml_edit::DocumentMut` 只修改目标叶子项，保留原注释、排序与未知字段；修改后的完整文本必须先成功反序列化为 `SushiConfig`，再通过同目录临时文件同步并原子替换目标文件。已有 Unix 权限位在替换后保持不变。任何键、类型、范围、解析或写入错误都不会改动原文件。

`jwt.secret` 的 get/set 均被拒绝。CLI 不提供通过参数读取或写入 secret 的覆盖选项；敏感值必须通过受保护的文件编辑或未来专用秘密管理入口处理。成功 set 只影响后续启动，输出明确要求重启，不修改当前 `ConfigStore`。

动态 CLI 授权将读取和写入区分为 `config:get` 与 `config:set` target，便于后续建立不同 policy binding。命令仍由 `host.cli` capability 提供，不扩大 bootstrap-safe 恢复面。

## 曾考虑的替代方案

**整体序列化 `SushiConfig`。** 该方案代码较少，但会丢失注释、字段顺序和当前版本尚不认识的字段，因此不用于用户配置文件编辑。

**允许任意点号键。** Serde 默认忽略未知字段，错误拼写可能写入成功但永不生效；显式白名单提供可诊断的 fail-closed 行为。

**允许 `config set jwt.secret`。** secret 会出现在 shell history 和进程参数中，即使 get 被隐藏仍不可接受，因此读写同时禁止。

**把 config 命令改为 bootstrap-safe。** 这会绕过 `host.cli` 的 capability 所有权并扩大恢复面；损坏配置继续通过 `doctor`、手工编辑或版本控制恢复。

## 验证

二进制回归测试覆盖实际 `--config` 路径、默认值读取、定点写入、注释与未知 section 保留、敏感键拒绝、未知键和越界端口失败不改文件，以及 `runtime.profile null` 删除。CLI 单元测试覆盖缺失文件不创建、Unix 权限保持和 `config:get` / `config:set` 授权目标。

## 后果

配置管理从占位命令变为可预测的持久化接口，且不把 CLI 变成任意 TOML 或秘密管理器。代价是新增字段必须显式加入键白名单；`jwt.secret` 仍需通过安全文件编辑管理；写入后必须重启进程才能生效。`toml_edit` 与临时文件依赖增加少量构建体积，但换取了注释保留、原子替换和清晰的失败边界。
