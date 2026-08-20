# Sushi CLI 配置读写闭环计划

```yaml
selected_mode: complex
goal: >-
  将 sushi config get/set 从占位输出实现为安全、可测试的配置文件管理命令，
  使用实际 --config 路径，保留 TOML 注释与非目标字段，并避免敏感配置泄露。
next_action: complete
```

## 范围

### 包含

- `sushi config get <KEY>` 读取当前启动实际生效的已知配置项。
- `sushi config set <KEY> <VALUE>` 修改 `--config` 指定的配置文件。
- 使用点号键语法，例如 `server.port`、`runtime.profile`。
- 对数值、字符串和 nullable profile 做类型校验。
- 写入前验证修改后的完整文档可以反序列化为 `SushiConfig`。
- 同目录临时文件原子替换，已有文件权限尽量保持不变。
- 保留原 TOML 的注释、字段顺序和非目标字段。
- `jwt.secret` 的读取与写入都拒绝，避免值进入终端、日志、shell history 或进程参数。
- 增加命令级、文件持久化和二进制回归测试。
- 同步配置指南与 Agent Note。

### 排除

- 任意未知 TOML 键写入。
- 批量导入、删除整个 section 或交互式编辑器。
- 运行中热重载数据库、监听地址、JWT 或 profile。
- 通过 Admin HTTP 页面写配置。
- 修改当前工作区未提交的 `config.toml` 本地端口值。

## 验收条件

1. `sushi --config <PATH> config get server.port` 输出当前生效端口。
2. 缺失但有默认值的键通过当前 `SushiConfig` 快照返回默认值。
3. `config set server.port 4100` 只修改目标键，保留相邻注释和未知字段。
4. `runtime.profile` 接受普通字符串和 `null`；`null` 从配置中移除该可选值。
5. 未知键、非法数值、越界端口和空字符串约束 fail closed，原文件字节不变。
6. `config get/set jwt.secret` 返回稳定敏感字段错误，不打印或持久化命令行中的 secret。
7. 写入使用 launcher 解析到的实际 `--config` 路径，不隐式修改仓库根 `config.toml`。
8. set 成功后提示修改已持久化且需要重启；不声称当前 runtime 已热更新。
9. 目标测试、CLI baseline、`cargo test --workspace -q`、格式和差异检查全部通过。

## 决策

### 键集合

首版只支持 `SushiConfig` 的叶子字段：

- `server.host`
- `server.port`
- `server.body_size_limit`
- `database.path`
- `jwt.secret`
- `jwt.access_ttl`
- `jwt.refresh_ttl`
- `plugins.directory`
- `file_browser.root_dir`
- `web.templates_dir`
- `web.static_dir`
- `web.static_url_prefix`
- `runtime.profile`
- `runtime.profiles_dir`
- `runtime.bundles_dir`

白名单避免拼写错误被 TOML/Serde 当作无效但可解析的未知字段，从而制造“写入成功但运行时不生效”的假象。

### Get 语义

`get` 从已启动 `SushiContext` 的 `ConfigStore` 读取，因此包括默认值，并与本次命令实际使用的配置快照一致。输出使用 TOML 标量形式；字符串带引号，数值直接输出，`runtime.profile` 未设置时输出 `null`。

`jwt.secret` 是唯一敏感字段。首版不增加显示或设置 secret 的逃生参数，显式拒绝 get/set，避免终端、日志、shell history 和进程参数捕获泄露。

### Set 语义

`set` 要求目标配置文件已存在，再读取原始 TOML 为 `toml_edit::DocumentMut`，只替换目标叶子项。修改后的文本必须再次解析为 `SushiConfig`，随后通过配置目录中的临时文件持久化并原子替换目标路径。缺失文件继续由用户显式创建，避免在完整 runtime bootstrap 已使用默认值后产生意外新文件。

成功写入只影响后续进程。当前命令已基于旧配置完成 bootstrap，因此输出明确的重启提示，不更新内存快照。

## 曾考虑的替代方案

**整体序列化 `SushiConfig`。** 实现简单，但会丢失注释、字段顺序和当前二进制尚不认识的字段，不适合作为用户配置编辑器。

**允许任意点号键。** 更灵活，但未知字段默认会被 Serde 忽略，拼写错误可能静默写入且永不生效；首版选择显式白名单。

**让 config 命令 bootstrap-safe。** 这样可修复损坏配置，但会绕过 `host.cli` profile capability 所有权，并扩大动态 CLI 的恢复面。本任务保持现有产品契约，损坏配置继续使用 `doctor`、手工编辑或版本控制恢复。

## 文件地图

- `crates/sushi-cli/src/commands/config_cmd.rs`：键定义、读取、类型解析和原子写入。
- `crates/sushi-cli/src/builtin.rs`：把实际 config path 与 `SushiContext` 捕获进 command handler。
- `crates/sushi-cli/src/app.rs`：构造 `HostCliFactory` 时传递 config path。
- `crates/sushi-cli/src/commands/dynamic.rs`：细化 config get/set authorization target。
- `crates/sushi/tests/cli_baseline.rs`：真实二进制与临时配置回归。
- `Cargo.toml`、`Cargo.lock`、`crates/sushi-cli/Cargo.toml`：`toml_edit` 与原子临时文件依赖。
- `docs/wiki/guides/configuration.md`：用户命令、键语法和重启语义。
- `.mozi/notes/implemented/feature/2026-08-20-cli-config-file-editing.md`：配置文件编辑安全决策。

## 实施步骤

1. 先为 get、set、敏感键和失败不改文件增加回归测试。
2. 增加已知键解析和 typed value 转换。
3. 使用 `toml_edit` 更新目标叶子项并验证完整配置。
4. 使用同目录临时文件原子替换并保留文件权限。
5. 将 launcher 的实际 config path 接入 `HostCliFactory` handler。
6. 增加 `config:get` / `config:set` authorization target。
7. 同步配置指南、Agent Note 与验证记录。

## 风险与缓解

- **敏感值泄露：** `jwt.secret` 的 get/set 都明确拒绝，错误不得包含原值或命令行输入值。
- **文件损坏：** 先内存解析和完整配置校验，成功后才原子替换。
- **注释丢失：** 使用 `toml_edit` 只修改目标节点，不整体序列化。
- **写错文件：** config path 由动态 launcher 捕获并传入 handler；测试使用临时路径。
- **热重载误解：** set 输出明确要求重启，不修改当前 `ConfigStore`。

## 回滚

- 不涉及数据库 schema 或数据迁移。
- 代码回滚不会修改已写入配置；配置文件本身仍是标准 TOML，可由版本控制或手工恢复。
- 若原子写入实现出现平台问题，可保留 typed 编辑与验证，单独替换持久化适配器。

## 实施结果

- `HostCliFactory` 捕获动态 launcher 实际解析的配置路径，并把同一 `SushiContext` 配置快照交给 get handler。
- `config get` 支持已知叶子字段和默认补全值；字符串以 TOML 字面量输出，`runtime.profile` 未设置时输出 `null`。
- `config set` 使用 `toml_edit` 定点修改、完整 `SushiConfig` 校验、同目录临时文件同步和原子替换；保留注释、未知 section 与已有权限。
- `jwt.secret` 的 get/set 均拒绝；未知键、缺失文件、非法类型与越界端口 fail closed。
- 写入成功只提示重启后生效，不修改当前 runtime 配置快照。
- 动态 CLI authorization target 已细分为 `config:get` 与 `config:set`。

## 最终验证记录（2026-08-20）

- `cargo test -p sushi-cli --lib -q`：31 passed；覆盖键解析、空白字符串、缺失 section、缺失文件、权限保持和授权 target。
- `cargo test -p sushi --test cli_baseline -q`：14 passed；覆盖实际配置路径、默认值、定点写入、安全失败和 nullable profile。
- `cargo test --workspace -q`：通过，所有 workspace test target 成功退出。
- `cargo fmt --all -- --check`、`git diff --check` 和 Agent Note validator：通过。
- 当前工作区原有 `config.toml` 端口差异保持不变，未被测试或实现写入。
