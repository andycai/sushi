# 一切皆插件运行时缺口闭环计划

```yaml
selected_mode: complex
goal: >-
  清理一切皆插件权威工件中的过期内容，补齐 profile 控制的动态 CLI、
  bootstrap-safe version、临时 overlay、唯一生命周期入口、强类型 package identity、
  完整生命周期状态与 PluginRepository 职责边界，并以自动测试证明契约闭环。
next_action: complete
```

## 范围

### 包含

- 清理 `.mozi/artifacts/everything-plugin-runtime/2026-08-18-everything-plugin-runtime.md` 中与当前实现不符的完成声明、历史兼容步骤、虚构文件地图和已放弃的 HTTP 恢复接口承诺。
- 让 `builtin:host-cli` 的 profile entry 真正决定业务 CLI command capability 是否存在。
- 让 `--version` 在 profile、数据库或插件损坏时仍可直接输出。
- 增加可重复的 `--overlay-file <PATH>`，按参数顺序在 profile overlay 后完整替换 entry。
- 移除公开的 `PluginManager::set_plugin_enabled` 生命周期旁路，治理调用统一经过 `SushiContext`/`RuntimeHost`。
- 将 package 级稳定 identity 接入 migration、插件状态和资源规格的核心边界。
- 让 `Discovered` 与 `Migrating` 成为真实可观察状态，并把插件元数据持久化职责移入 `PluginRepository`。
- 同步用户文档、Agent Note 和验证记录。

### 排除

- Rust 动态库 ABI、WASM、远程插件市场和 SQL down migration。
- Lua 同包多实例。
- runtime/Lua/host 的物理 crate 拆分。
- Admin 前端重写或新增 HTTP 恢复接口。

## 验收条件

1. 不含 `host.cli` entry 的合法 profile，其 `sushi --help` 不显示 `serve/plugin/config/seed` 等业务命令；包含该 entry 的 shipped profile 保持现有命令集合。
2. `sushi --config <无效配置> --version` 成功输出版本，不进行 profile 解析、数据库打开或插件执行。
3. `--overlay-file` 可按顺序覆盖已存在 entry；未知 target、重复 entry、无效 source 和 required disable 均 fail closed；`inspect profile` 展示最终值与 `cli-overlay:<path>` 来源。
4. 生产代码不存在可直接修改 enable intent 而不执行 lifecycle 的公开入口；optional enable/disable 仍完成 activate/deactivate，required toggle 仍拒绝。
5. migration descriptor、插件状态和 template/static package 资源在公共核心结构中使用 `PluginId`，序列化和数据库边界显式转换为字符串。
6. lifecycle 查询可观察 `Discovered`、`Migrating`、`Activating`、`Active`、`Deactivating`、`Inactive` 与 `Failed`；迁移失败不伪装成 active。
7. `PluginManager` 不再直接拥有插件元数据、required 集合和状态 repository，这些职责由 `PluginRepository` 承担；crate 物理拆分仍非门禁。
8. 目标测试、`cargo test --workspace -q`、格式检查、`git diff --check` 和 Agent Note validator 全部通过。

## 决策

### CLI 临时覆盖格式

采用可重复参数 `--overlay-file <PATH>`。每个文件使用 schema v1、声明 `[[overlays]]`，entry 结构与 profile overlay 相同。覆盖顺序为 bundle 声明顺序、profile overlay、命令行文件顺序；每个覆盖替换完整 entry，不做深合并。

选择文件而不是多组 `--set` 字符串，是为了复用 TOML parser、避免临时语法绕过 source/grant 校验，并让诊断输出保留可审计来源。

### 恢复面

Host 不新增 HTTP 恢复接口。恢复面固定为不执行插件入口的 `doctor` 和 `inspect profile`，以及不 bootstrap 的 `--version`。这避免在产品 Router 中引入未定义的匿名治理协议。

### CLI capability 所有权

`HostCliFactory` 在其 owner staging 中注册 builtin command specs。Launcher 只解析 bootstrap-safe 入口并 dispatch snapshot，不再无条件枚举业务命令。`doctor` 和 `inspect profile` 仍由第一阶段 parser 处理，因此在 `host.cli` 缺失或 runtime 失败时可用。

### Repository 边界

新增 `PluginRepository` 封装插件信息、required 标记和 `PluginStateRepository`。`PluginManager` 保留 VM dispatch 与 capability facade；`RuntimeHost` 保留 lifecycle serialization。该拆分按职责移动现有逻辑，不制造新 crate 或第二份状态。

## 文件地图

- `.mozi/artifacts/everything-plugin-runtime/2026-08-18-everything-plugin-runtime.md`：权威契约、实时状态和验证记录。
- `crates/sushi-cli/src/commands/dynamic.rs`：bootstrap parser、version、overlay 和 snapshot dispatch。
- `crates/sushi-cli/src/app.rs`：带 overlay 的 profile resolution/bootstrap。
- `crates/sushi-core/src/runtime/profile.rs`：CLI overlay 文档解析与组合。
- `crates/sushi-cli/src/builtin.rs`：由 `HostCliFactory` 发布 CLI capabilities。
- `crates/sushi-core/src/plugin/repository.rs`：插件状态与元数据边界。
- `crates/sushi-core/src/plugin/manager.rs`：移除旁路并委托 repository。
- `crates/sushi-core/src/runtime/identity.rs`、`migration.rs`、`registry.rs`：`PluginId` 强类型接线。
- `crates/sushi-core/src/lua/loader.rs`、`crates/sushi-core/src/runtime/lifecycle.rs`：完整状态机。
- `crates/sushi-core/tests/runtime_profile.rs`、`crates/sushi-core/tests/runtime_migrations.rs` 和 `crates/sushi/tests/cli_baseline.rs`：回归证据。
- `.mozi/notes/implemented/architecture/2026-08-18-everything-plugin-runtime-single-path-kernel.md`：同步长期事实。

## 实施步骤

1. 清理主工件和当前文档中的过期契约，删除历史等价清单。
2. 先增加 CLI profile gating、version 与 overlay 回归测试，确认缺口可复现。
3. 将 builtin command 注册迁入 `HostCliFactory`，launcher 只消费 snapshot。
4. 扩展 profile resolver 接受有序 overlay 文件，并接入 bootstrap-safe parser。
5. 删除 `PluginManager` 公开 toggle，建立 `PluginRepository` 并迁移元数据职责。
6. 将 `PluginId` 接入 migration、状态和 package 资源边界，显式处理字符串存储。
7. 接线 discovery/migration lifecycle 状态并增加失败路径测试。
8. 同步 Agent Note、权威文档和验证记录，执行目标与 workspace 验证。

## 风险与缓解

- **CLI 自举循环：** command handler 需要 runtime context，而 factory 在 bootstrap 中激活。通过让 factory 构造 handler 时捕获 `SushiContext` clone，并保留 `doctor/inspect profile` 的第一阶段旁路避免循环。
- **overlay 权限绕过：** 临时文件可能提升 grant。它仍经过与 profile 相同的 source、required、approval 和 grant 校验；来源明确标记为 CLI overlay，便于审计。
- **identity 改型扩散：** JSON、SQL 和 Lua 边界仍需要字符串。只在 package 级核心结构中强类型化，在边界调用 `as_str()`，避免一次性改写所有展示 DTO。
- **repository 移动回归：** 先保持 `PluginManager` 外部查询 API 不变，仅移动所有权和内部实现，再由现有治理测试锁定行为。
- **大工作区并发改动：** 不回退现有变更；每个补丁前读取实时文件，格式化只覆盖本轮修改的 Rust 文件。

## 回滚

- CLI gating、version 和 overlay 都是无 schema 数据变更的代码前滚，可按对应文件回退。
- `PluginRepository` 是内部职责移动，不改变数据库 schema；回滚恢复委托前字段即可。
- `PluginId` 的存储表示仍为原字符串，不执行数据迁移；回滚不会改变已有数据库内容。
- lifecycle 新状态只增加观测精度，不改变 capability commit/rollback 顺序。

## 最终验证记录（2026-08-20）

- `cargo test --workspace -q`：通过，所有 workspace test target 成功退出。
- `cargo test -p sushi-core --lib -q`：173 passed；覆盖状态机、唯一生命周期入口、owner registry 和 Lua activation。
- `cargo test -p sushi-cli --lib -q`：26 passed；覆盖 CLI gating、version、overlay 与 migration failure 状态。
- `cargo test -p sushi-api --lib -q`：28 passed；覆盖 API dispatch 与 disable 后 capability 撤销。
- `cargo test -p sushi-admin --test admin_web -q`：85 passed；覆盖 Admin 页面、workspace 和静态资源生命周期。
- `cargo fmt --all -- --check`、`git diff --check` 和 Agent Note validator：通过。
- 文档收口复核确认：`inspect profile` 只输出 bootstrap-safe profile 解析结果，`inspect capabilities` 追加 active registration 摘要；shipped `minimal` profile 继承 `host.cli`，保留 Host builtin commands，但不激活 API、Admin 或官方 Lua 产品能力。
