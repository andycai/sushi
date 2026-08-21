# Sushi “一切皆插件”运行时重构计划

```yaml
selected_mode: complex
goal: >-
  将 Sushi 从“宿主硬编码产品能力、Lua 插件作为旁路扩展”渐进重构为
  “最小可信内核、Rust/Lua 等价插件、配置化组合、owner 级可撤销注册”的平台，
  同时保持默认发行版的 Admin/API/CLI 行为和现有官方 Lua 插件兼容。
next_action: complete
```

> 批准状态：用户已于 2026-08-18 批准第 13 节全部五项架构取舍；实施从 Slice 0-1 开始。

> 实施进度（2026-08-20）：Slice 0 至 Slice 12 的行为缺口已完成本轮实现，crate 物理拆分仍明确延后。运行时具备 owner-scoped staged registry、真实 activate/deactivate/reload、动态 template/static/event/task 生命周期、profile/bundle 组合、统一 HTTP/Admin dispatcher、plugin migration catalog、required Rust builtin 插件、动态 CLI launcher、强类型 `PluginId` 边界、真实 `Discovered`/`Migrating`/`Failed` 状态，以及 source/path trust + manifest request + profile grant 权限交集。

> 本轮推进（2026-08-19）：builtin 发现、activation 与 migration descriptor 均统一进入 `BuiltinFactoryRegistry`，bootstrap 不再按产品 key 写 activation/migration 条件分支。Lua task 通过 `sushi.task.spawn/interval` 延迟到 capability/VM commit 后启动；disable 按 owner 取消，successful reload 按上一代 task registration IDs 定向取消，failed reload 保留旧 generation。所有动态 CLI 命令共用 policy authorization；profile grant 显式要求 `approved = true`；`serve --api-only/--admin-only` 已删除，Ctrl-C/SIGTERM 会触发 HTTP drain 与全量 owner task 清理；`inspect capabilities` 输出 owner 与 registration source。

> 缺口闭环（2026-08-19）：API/Admin surface 现由 owner-scoped `transport:api/admin` capability 选择，`serve.rs` 不再检查产品 builtin key；未批准 Lua entry 不执行任何插件代码或 effect，required 未批准条目在打开数据库前失败；legacy 直接注册 adapter 发出可审计 deprecation warning；Rust builtin factory 通过 `PluginContext` 注册 deferred owner task；`doctor` 只读检查 manifest、approval、migration checksum/bridge/recovery 并输出来源和修复建议；四 shipped profile HTTP `200/401/403/404`、optional disable/enable 与在途 drain 已有自动测试。精确目标测试与最终 workspace 门禁均已通过，实施进入代码审查阶段。

> 审查修复（2026-08-20）：未批准 optional Lua entry 已从 migration catalog 排除；插件静态资源与模板读取增加 canonical containment，拒绝目录内指向根外的符号链接；静态资源 Router 提升为 Host transport，API-only profile 的公共插件页面可加载全局与插件 CSS/JS；动态 CLI launcher 只 bootstrap 一次并复用同一 `SushiContext` 执行 `serve`、`plugin`、`seed` 与 `inspect capabilities`；根帮助测试改用临时配置和临时数据库。对应回归测试与最终 workspace 门禁均已通过，进入复审阶段。

> 缺口复审（2026-08-20）：`host.cli` gating、bootstrap-safe `--version`、有序 `--overlay-file`、唯一 lifecycle 入口、`PluginId` 核心边界、真实生命周期状态和 `PluginRepository` 职责拆分均已落地；本轮收口转入最终全量验证。完成状态以本工件最新验证记录为准。

> 最终收口（2026-08-20）：行为验收、文档事实与 shipped profile 投影已完成复核。`inspect profile` 只解析并输出最终 profile，不打开数据库；`inspect capabilities` 在同一输出后追加 active capability key、owner 与 registration source。`minimal` profile 继承 `base` bundle 中的 required `host.cli`，因此保留 `serve`、`plugin`、`config`、`seed` 和 bootstrap-safe `inspect`/`doctor`，但不激活 API、Admin 或官方 Lua 产品能力。计划状态更新为 `complete`。

## 1. 背景与当前结论

当前系统已经具备插件 manifest、Lua VM、API/Admin/CLI handler 注册、权限、策略、插件状态与资源隔离等基础能力。经过本轮迁移，产品能力由 profile 选择的 builtin/Lua plugin activation 提供；保留的边界均为有意的宿主或语法适配边界：

- `crates/sushi-cli/src/app.rs` 仍负责 bootstrap 编排，这是宿主生命周期边界而非产品 capability 定义。
- `crates/sushi-core/src/lua/bindings.rs` 仍提供旧 `sushi.api/cli/admin/web` 语法，但它们直接写入 `__contract_registry`，不形成第二注册路径。
- `PluginManager::register_*` legacy facade、独立 API/Admin/CLI registry 和 `RegistrationSource::Legacy` 已删除；Rust/Lua 测试与生产代码都通过 staged registry 注册。插件元数据、required 集合和持久化状态由 `PluginRepository` 承担。
- `crates/sushi/src/main.rs` 已仅负责 tracing 初始化和动态 CLI launcher 调用，不再定义业务命令枚举。

治理入口 `SushiContext::set_plugin_enabled(...)` 委托 RuntimeHost 执行完整 optional plugin lifecycle；不得公开只修改 enabled intent 的 manager 入口。required plugin 仍只能通过 profile 变更和受控重启调整。默认 profile 缺失时 resolver fail closed，不再扫描插件目录生成隐式产品组合。

本计划借鉴 deepseek-harness/Cordis 的机制，而非复制其 TypeScript 目录：

1. 运行实例由有序 profile/bundle 组合产生。
2. 插件只通过受限上下文贡献 service、capability、event 和可撤销 effect。
3. 每项注册都有 owner，插件加载失败或卸载时可整体撤销。
4. 首方功能同样以插件注册，宿主只保留安全和 transport 边界。

## 2. 目标

### 2.1 平台目标

- 可信内核只拥有配置解析、插件发现/解析、权限强制、生命周期、审计、迁移执行、运行时隔离和 transport dispatch。
- Rust 与 Lua 插件使用同一组 transport-neutral contract 注册 API、Admin、CLI、静态资源、模板、事件和后台任务。
- 默认产品由 profile 组合，不再由 `serve.rs` 和 `main.rs` 硬编码决定。
- 注册过程具有事务性：插件激活期间先进入 staging，全部校验通过后原子发布；失败时不泄漏半注册状态。
- 插件实例卸载时，所有 owner 相关注册可撤销；新请求立即看不到旧能力，已取得旧 snapshot 的在途请求允许完成。
- optional 插件可以在不重启进程的情况下 disable/enable；required 系统插件只能通过 profile 变更和受控重启调整。
- 数据库 migration 具有 owner、稳定 ID、checksum 和原子记录，未来不再向中央 bootstrap 追加产品 migration 常量。

### 2.2 兼容目标

- 默认 profile 启动后的现有 API 路径、Admin 页面、HTMX fragment、静态 URL、模板逻辑名和 Lua `sushi.*` API 保持兼容。
- 现有 `plugins/official/*` 和 `plugins/third_party/_example` 在兼容期无需一次性改写。
- 当前 policy key、public route、body size、插件禁用错误和响应 envelope 语义保留，随后收敛为统一 dispatcher。
- 现有 `config.toml` 继续可用；未设置 profile 时默认选择 `default`。

## 3. 范围

### 3.1 Include

- 统一 capability model、owner identity、staging transaction、immutable snapshot 和 dispose 生命周期。
- `PluginManager` 兼容 facade 与 Lua loader 迁移。
- profile、bundle、overlay 和 builtin/Lua plugin source 解析。
- 动态 HTTP/Admin fallback dispatch、动态插件静态资源和模板根。
- 两阶段 CLI 解析和动态命令注册。
- Rust 首方插件 SDK，以及现有内置 API/Admin/CLI 功能的渐进迁移。
- 插件 migration catalog、checksum、existing database bridge 和 fresh-install 流程。
- 插件信任来源与 capability ceiling 的安全收敛。
- 文档、示例、测试和诊断输出。

### 3.2 Exclude

- 第一阶段不实现 Rust 动态库 ABI、WASM 插件或远程插件下载/市场。
- 不复制 Cordis 的全部 service proxy、fiber、HMR 或 JavaScript 表达式配置机制。
- 不在第一阶段支持任意 Axum extractor/middleware 对象由动态插件直接注入；插件通过规范化 request/response contract 工作。
- 不承诺 SQL migration 自动 down；数据库变更继续采用 forward-only 策略。
- 不在 capability contract 稳定前重命名或拆分全部现有 crate。
- 不改变 Admin 前端技术栈，也不重写现有 HTMX/Alpine 页面。
- 不在首轮支持同一个 Lua package 的多实例挂载；identity 先正确区分 package 与 instance，但 loader 暂时拒绝重复 Lua package instance。

## 4. 验收条件

### 4.1 运行时与生命周期

- 每个 runtime registration 都包含 `PluginInstanceId` owner，无法创建无 owner 的插件注册。
- 插件激活失败后 registry snapshot、VM、模板根、静态根、事件订阅和后台任务均无残留。
- disable optional 插件后，新 API/Admin/CLI 请求不可再命中该插件，静态文件和模板不可再解析，`loaded=false`；在途请求允许完成。
- enable 同一插件后，无需进程重启即可重新加载 VM、重新提交 registration 并恢复能力。
- 重复 route/command/page 注册默认 fail closed，错误包含双方 owner 与 capability key。

### 4.2 组合与可诊断性

- `default`、`api`、`admin`、`minimal` profile 能产生确定性 entry 顺序和 snapshot。
- bundle 按声明顺序应用，profile overlay 按稳定 entry ID 替换完整 entry 配置；重复 ID、缺失 bundle、未知 builtin factory 均在启动前失败。
- `sushi inspect profile` 在不打开数据库的情况下列出 entry ID、source、enabled/required、最终 config/grants 和来源；`sushi inspect capabilities` 在该 profile 输出后追加 active registration key、owner 与 registration source 摘要。
- `serve --api-only` / `--admin-only` 已从 Clap surface 删除并由二进制回归测试锁定；产品组合只接受全局 `--profile`。

### 4.3 Surface 一致性

- Rust 与 Lua API route 通过同一个 matcher、鉴权、策略、body limit、日志和错误映射管线。
- Rust 与 Lua Admin 页面使用同一 capability contract，并保留现有 fragment/full-page contract。
- CLI 动态 help 能显示首方和 Lua 命令；命令授权使用同一 policy binding。
- 默认 profile 下现有 Admin/API/CLI 回归测试全部通过。

### 4.4 数据与安全

- 每个新 migration 记录 `(plugin_id, migration_id, checksum, applied_at)`，SQL 与记录位于同一 SQLite transaction。
- 已应用 migration 的 checksum 改变时启动失败，不静默重跑。
- existing database 可由 bridge 识别 `001-008` 历史状态；fresh database 根据 profile 所需插件执行 migration catalog。
- manifest 不再能够通过自报 `kind = "official"` 获得全权限；最终有效能力是宿主信任、manifest ceiling、profile grant 和管理员批准的交集。
- required 系统插件不能通过普通 Admin/CLI toggle 禁用，并存在 bootstrap-safe recovery/inspect 入口。

### 4.5 完成证据

- 针对 owner lifecycle、profile composition、HTTP dynamic dispatch、CLI dynamic dispatch、migration runner 和 trust policy 的新测试通过。
- `cargo test -p sushi-core --test template_service -q` 通过。
- `cargo test -p sushi-admin --test admin_web -q` 通过。
- `cargo test --workspace -q` 通过。
- `cargo fmt --all -- --check` 通过；若仓库引入 Clippy gate，再运行对应命令。

## 5. 关键决策与真实方案比较

### 5.1 总体迁移策略

#### 方案 A：一次性拆出新 runtime crate 并搬空宿主

- 优点：目标结构快速可见，旧抽象不会长期存在。
- 缺点：会同时改变 crate 依赖、bootstrap、路由、CLI、migration 和插件 ABI，回归定位困难；当前 6 个中心文件合计超过 6500 行，风险不可接受。

#### 方案 B：在 `sushi-core` 内建立新 kernel，通过 facade 绞杀旧路径（推荐）

- 先建立新 identity/registry/snapshot/lifecycle；`PluginManager` 暂时委托新内核。
- Lua 与三个 surface 逐条迁移；每一阶段保持 workspace 可运行。
- capability contract 稳定后再决定是否物理拆分 `sushi-runtime`、`sushi-plugin-sdk` 等 crate。

#### 方案 C：只增加 profile，不改变 lifecycle 和 registry

- 能减少 `serve.rs` 条件分支，但禁用、重新启用、半注册失败、双路由鉴权和 migration ownership 仍然存在。
- 不满足“一切皆插件”的核心目标。

**决策：选择方案 B。** crate 物理边界不是首个关键路径，owner-scoped capability contract 才是。

### 5.2 动态 HTTP 路由策略

#### 方案 A：每次插件变化重建并热交换完整 Axum Router

- 保留 Axum 原生 route/extractor，但需要安全交换 Service、处理 state 类型、在途请求和 static nest，复杂度高。

#### 方案 B：稳定 Host Router + immutable capability snapshot dispatcher（推荐）

- Host 只保留健康检查、bootstrap/recovery、全局静态入口等不可插件化边界。
- unmatched HTTP 请求进入统一 dispatcher；snapshot matcher 决定 Rust/Lua handler、policy、public 和 owner。
- enable/disable 只需原子发布新 snapshot，不需要替换监听器。
- 现有 Lua handler 本来已使用 method/path/body 通用分发，因此迁移成本较低。

#### 方案 C：所有 plugin state 变化要求重启

- 最简单，但与当前 Admin toggle 的用户预期和 owner lifecycle 目标冲突。

**决策：选择方案 B。** 第一版 matcher 只承诺当前精确路径和尾部 `*` 语义；参数化路径作为后续 contract version，不顺便扩大范围。

### 5.3 Rust 插件交付形式

#### 方案 A：稳定动态库 ABI

- 真正可外部安装，但 Rust ABI、版本兼容、崩溃隔离和供应链安全成本高。

#### 方案 B：静态链接 builtin factory + 等价 RuntimePlugin contract（推荐）

- 首方 Rust 插件由 workspace crate/module 实现，profile 使用 `builtin:<key>` 引用。
- 第三方继续优先使用 Lua；未来可在不改变 capability contract 的前提下增加 WASM/动态库 source adapter。

**决策：选择方案 B。** “一切皆插件”定义为运行时组合和生命周期一致，不等同于“所有 Rust 代码必须动态加载”。

### 5.4 Profile overlay 语义

#### 方案 A：递归 deep merge

- 写起来短，但数组、删除字段、类型变化和来源诊断容易产生隐式行为。

#### 方案 B：按 entry ID 替换完整 entry/config（推荐）

- 与参考项目相近，可预测、可 dump、易于审计；覆盖方需要重述保留字段。

**决策：选择方案 B。** schema 明确区分 bundle 插入与 overlay 替换，未知 target 默认报错。

### 5.5 插件禁用语义

- `optional`：允许 Admin/CLI runtime toggle，执行完整 deactivate/activate。
- `required`：profile 声明的系统插件；运行时治理接口拒绝 toggle，只能修改 profile 后受控重启。
- `discovered` 不等于 `active`；只有 profile entry 或兼容 discovery bundle 选择的插件才激活。

该决策避免认证、策略或插件治理界面被自身禁用后无法恢复。

### 5.6 Migration 所有权

- migration owner 是稳定 `PluginId`，不是可重复挂载的 `PluginInstanceId`。
- 历史 `001-008` 不修改 checksum；通过 bridge catalog 映射既有记录。
- 新 migration forward-only；代码/profile 回滚不自动回滚数据。
- migration 在 plugin activation 之前运行，但只有经过信任和 database grant 校验的 entry 可以声明 migration。

## 6. 目标架构与接口

### 6.1 Identity

```rust
pub struct PluginId(String);          // package: official/cms, builtin/auth
pub struct PluginInstanceId(String);  // profile entry: cms.default
pub struct RegistrationId(u64);

pub enum PluginSource {
    Builtin { key: String },
    Lua { path: PathBuf },
}
```

- `PluginId` 负责信任、migration、安装状态和 package 级资源。
- `PluginInstanceId` 负责本次挂载的 config、生命周期和 registration owner。
- 第一版 Lua adapter 校验同一 `PluginId` 最多一个 active instance，但不把限制写进核心 identity。

### 6.2 Plugin contract

最终实现使用两个等价 adapter，而不是强制一个物理 trait：

- 受信 Rust builtin 实现 `BuiltinPluginFactory`，由静态 `BuiltinFactoryRegistry` 按 profile key 发现；factory 提供 activation，并可提供 owner-local migration descriptor。
- Lua package 由 `LuaPlugin` 适配，activation 只拿按 profile config/grants 构造的 `PluginContext`；数据库、文件、模板、事件与 task 都由受限 gateway 提供。
- 两类 adapter 最终都向带 `PluginInstanceId` 的 staged registrar 注册 transport capability；成功后 Runtime 一次 commit，`PluginHandle` 持有 runtime generation、registration IDs、task IDs 和 cancellation。
- Builtin factory 属于可信宿主边界，可以接收 `SushiContext`；Lua 插件不能取得完整 host context。

### 6.3 Capability registry

首版统一以下 capability：

- `HttpRouteSpec`：method、path pattern、surface、public/policy、body limit、handler reference。
- `AdminPageSpec`：path、title、render mode、policy、asset bundles、handler reference。
- `CliCommandSpec`：name、description、policy、argument mode、handler reference。
- `TemplateRootSpec`、`StaticRootSpec`。
- `EventSubscriptionSpec`、`MenuContributionSpec`。

每项规范化记录至少包含：

```rust
pub struct OwnedRegistration<T> {
    pub id: RegistrationId,
    pub owner: PluginInstanceId,
    pub source: RegistrationSource,
    pub value: T,
}
```

Registry API 需要支持：

- `stage(owner)` 创建隔离 transaction。
- `validate(staged, current)` 检查冲突、权限、policy scope 和 reserved path。
- `commit(staged)` 发布新 immutable `CapabilitySnapshot`。
- `remove_owner(owner)` 发布不含该 owner 的新 snapshot。
- `inspect()` 输出 deterministic registration map。

实现首选 `Arc<RwLock<Arc<CapabilitySnapshot>>>`；请求只短暂读取并 clone `Arc`。若基准证明锁有问题，再考虑 `arc-swap`，不提前引入依赖。

后台 task 与 policy binding 是 owner-scoped effect，不是 transport capability，因此不放入 `CapabilitySnapshot`。Task 使用独立 `TaskRegistry`，但由 `PluginHandle` 记录 registration IDs 并遵守相同 owner/generation activate、reload、deactivate 与 shutdown 生命周期。

### 6.4 生命周期状态机

```text
discovered
  -> resolved
  -> migrating
  -> activating (staged only)
  -> active (snapshot committed)
  -> deactivating (new snapshot removes owner)
  -> inactive

activating/migrating failure -> failed (no visible registration)
```

Deactivate 顺序：

1. 从新 snapshot 移除 owner，阻止新 dispatch。
2. 标记 instance generation retiring。
3. 触发 cancellation，停止事件消费和后台任务。
4. 等待有界 drain；超时记录审计并继续释放。
5. 删除 VM/runtime instance、模板根和静态根引用。
6. 持久化 `loaded=false` 和 lifecycle event。

在途请求持有旧 snapshot 和 runtime `Arc`，允许完成；因此不会发生 handler 查到但 VM 已被提前释放的问题。

### 6.5 Profile schema

建议采用独立于运行参数的 `profiles/*.toml` 和 `bundles/*.toml`：

```toml
schema_version = 1
name = "default"
bundles = ["base", "api", "admin", "official"]

[[overlays]]
id = "file-browser.default"
enabled = true
required = false
source = "lua:official/file-browser"

[overlays.config]
route_prefix = "/app/files"
```

bundle 中使用相同 entry 结构。最终 entry 至少包含：

- `id`：稳定 `PluginInstanceId`。
- `source`：`builtin:<key>` 或 `lua:<relative-path>`。
- `enabled`、`required`。
- `config`：插件私有 JSON/TOML value。
- `grants`：宿主授予的 capability ceiling。

组合顺序：bundle 声明顺序 → profile overlays → 按参数顺序加载的 `--overlay-file <PATH>`。overlay 替换完整 entry；`sushi inspect profile` 展示最终值和最后来源。

### 6.6 HTTP host

- Host Router 保留健康检查、favicon 和 bootstrap-safe assets；其余请求进入统一 fallback dispatcher。恢复面由不执行插件入口的 `doctor`、`inspect profile` 和 `--version` 提供，不新增匿名 HTTP recovery 协议。
- Dispatcher 从 snapshot 进行 method/path 匹配，先检查 plugin active generation，再执行 public/auth/policy/body limit，最后调用 handler adapter。
- Rust handler 与 Lua handler 都实现 transport-neutral `HttpHandler`；Axum request 在 host 边界转换为规范化 request，handler response 再转换回 Axum response。
- 静态插件资源改为动态 handler：按 snapshot 查找 `StaticRootSpec`，执行 safe join 后读取；不再启动时 `nest_service` 固化所有 root。
- Host 无条件挂载独立 static Router；Admin Router 不再独占静态资源，API-only public web plugin 同样可以加载全局与插件 CSS/JS。
- Admin workspace 和 plugin public web routes 使用同一 dispatcher，但 `surface` 决定默认鉴权和响应约束。

### 6.7 CLI host

- 第一阶段保留 bootstrap-safe global args：`--config`、`--profile`、`--overlay-file`、`--role`、`--version`、`inspect profile`/`doctor`。
- 两阶段解析：先解析 global/bootstrap args，加载 profile 和 registry，再由 `CliCommandSpec` 构建完整 Clap command tree并解析剩余参数。
- launcher 只执行一次 bootstrap；所有需要运行时状态的 builtin handler 捕获并复用该 `SushiContext`，命令完成后由 launcher 统一移除 CLI owner capability 并 shutdown。
- Lua 兼容 command 使用 raw trailing args；Rust builtin command 可以声明结构化参数 schema。
- `serve`、`seed`、`plugin`、`config` 是 `builtin:host-cli` 发布的 command capability；launcher 不枚举业务命令，缺少 `host.cli` entry 时这些命令不进入 snapshot 或 help。

### 6.8 Template 与 static lifecycle

- `TemplateService` 的 loader 改为读取 owner-scoped root registry，而不是构造时捕获不可变 `HashMap`。
- 同步 Minijinja loader 使用短时 `std::sync::RwLock` 或等价无 async 等待的数据结构；模板与静态文件读取同时 canonicalize 注册根和目标文件，只允许根内普通文件。
- logical template name 和现有 `/static/plugins/<path-id>/...` URL 在兼容期保持不变。

### 6.9 Migration runner

新表建议为：

```sql
CREATE TABLE plugin_migrations (
    plugin_id TEXT NOT NULL,
    migration_id TEXT NOT NULL,
    checksum TEXT NOT NULL,
    applied_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (plugin_id, migration_id)
);
```

- `SqliteStorage` 增加 transaction 内执行 SQL 和记录 migration 的专用 API，不能继续由调用方分别 `execute_batch` 与 insert。
- `BuiltinPluginFactory::migrations` 提供 owner-local descriptor；registry 只汇总 profile 中 enabled builtin entry。Lua package 仅在 entry enabled 且 `approved = true` 时从受约束的 `migrations/*.sql` 读取，按 migration ID 排序。
- bridge 读取 `_sushi_migrations`，把历史 `001-008` 映射到 catalog owner；不得修改历史 SQL 内容。
- fresh-install 测试覆盖不同 profile，确保未加载业务插件不会执行其未来 migration。

## 7. 文件地图

以下是计划中的主要文件范围；实际实现可在不改变契约的情况下细化文件名。

### 7.1 运行时内核（保留在 `sushi-core`）

- `crates/sushi-core/src/runtime/mod.rs`
- `crates/sushi-core/src/runtime/identity.rs`
- `crates/sushi-core/src/runtime/lifecycle.rs`
- `crates/sushi-core/src/runtime/registry.rs`
- `crates/sushi-core/src/runtime/profile.rs`
- `crates/sushi-core/src/runtime/migration.rs`
- `crates/sushi-core/src/runtime/builtin.rs`
- `crates/sushi-core/src/runtime/task.rs`
- `crates/sushi-core/tests/runtime_registry.rs`
- `crates/sushi-core/tests/runtime_profile.rs`
- `crates/sushi-core/tests/runtime_migrations.rs`

### 7.2 修改：兼容层与 Lua adapter

- `crates/sushi-core/src/lib.rs`
- `crates/sushi-core/src/context.rs`
- `crates/sushi-core/src/plugin/mod.rs`
- `crates/sushi-core/src/plugin/manager.rs`
- `crates/sushi-core/src/plugin/state_repository.rs`
- `crates/sushi-core/src/lua/loader.rs`
- `crates/sushi-core/src/lua/adapters/*.rs`
- `crates/sushi-core/src/lua/bindings.rs`
- `crates/sushi-core/src/registry/event.rs`
- `crates/sushi-core/src/web/template_service.rs`
- `crates/sushi-core/src/storage/sqlite.rs`

### 7.3 修改：Host adapters

- `crates/sushi-api/src/router.rs`
- `crates/sushi-api/src/routes/*.rs`
- `crates/sushi-admin/src/router.rs`
- `crates/sushi-admin/src/routes/*.rs`
- `crates/sushi-admin/src/render.rs`
- `crates/sushi-cli/src/app.rs`
- `crates/sushi-cli/src/commands/*.rs`
- `crates/sushi/src/main.rs`

### 7.4 新增：组合配置

- `profiles/default.toml`
- `profiles/api.toml`
- `profiles/admin.toml`
- `profiles/minimal.toml`
- `bundles/base.toml`
- `bundles/api.toml`
- `bundles/admin.toml`
- `bundles/official.toml`

### 7.5 插件与 migration

- `plugins/official/*/plugin.toml`
- `plugins/official/*/migrations/*.sql`
- `plugins/third_party/_example/plugin.toml`
- `crates/sushi-core/src/runtime/migration.rs` 内的 migration catalog schema 与历史 bridge

### 7.6 文档与决策

- `docs/engineering/plugin-authoring-standards.md`
- `docs/engineering/coding-standards.md`
- `docs/wiki/architecture/plugin-runtime.md`
- `docs/wiki/guides/profile-composition.md`
- `docs/wiki/guides/plugin-lifecycle.md`
- `.mozi/memories/adr/0001-everything-plugin-runtime-owner-scoped-registry.md`
- `.mozi/memories/adr/0002-everything-plugin-runtime-profile-composition.md`
- `.mozi/notes/implemented/architecture/2026-08-18-everything-plugin-runtime-single-path-kernel.md`

两个 ADR 已在实施前创建；最终交付边界与长期防回退约束记录在 implemented Agent Note 中。

## 8. 实施步骤与迁移切片

每个切片应独立保持默认产品可运行；禁止在同一个 PR/commit 中同时进行大规模文件移动和行为变化。

### Slice 0：基线锁定与 ADR

**目标：** 固化当前行为，防止重构过程中以“目标架构”为由意外改变产品 contract。

1. 创建两个 ADR：owner-scoped registry；profile/bundle full replacement。
2. 增加 characterization tests：
   - Lua 插件注册 API/Admin/CLI 后当前 snapshot 内容。
   - disable 只阻止调用的现状，作为后续测试替换起点。
   - 默认 API/Admin 路径、静态 URL、模板逻辑名和 CLI 输出。
3. 以 shipped profile dump 和 capability golden 锁定默认组合，不维护会随实现漂移的手工等价清单。
4. 运行 workspace 基线并保存失败项；不得把既有失败混入架构重构。

**完成门槛：** 无产品行为改动；后续每个迁移项都有对应 characterization test。

**实施状态：完成。** Owner registry 与 profile composition ADR 已建立；默认 capability/migration/profile 投影、Lua contract、Admin/API/CLI 行为和插件治理均由 characterization/integration tests 锁定。后续迁移以这些测试和 workspace 基线为回归门禁。

### Slice 1：Owner identity 与事务化 registry

**目标：** 建立新内核最小闭环，不改现有启动方式。

1. 新增 `PluginId`、`PluginInstanceId`、`RegistrationId`、`PluginLifecycleState`。
2. 新增 staged registrar、owned registration、conflict error 和 immutable snapshot。
3. 先支持 `HttpRouteSpec`、`AdminPageSpec`、`CliCommandSpec`；静态/模板/event/task 在后续补齐。
4. `PluginManager` 增加新 kernel 字段，现有 `register_*` 方法转换为 legacy owner 下的新 registry 调用。
5. 现有 list/policy lookup 改读 snapshot；handler invocation 暂时继续使用旧 VM map。
6. 删除或标记废弃 `crates/sushi-core/src/registry/mod.rs` 的平行 API/Admin/CLI registry，避免出现第三套 source of truth。

**测试：** staged 内容 commit 前不可见；冲突 commit 不改变旧 snapshot；`remove_owner` 原子撤销；inspect 顺序稳定。

**回滚：** 保留旧 `PluginManager` API，若 snapshot 读取出现回归，可暂时切回旧 map；新 schema 尚无数据变更。

**实施状态：完成。** `PluginInstanceId`、`RegistrationId`、`PluginLifecycleState`、`StagedRegistrar`、`OwnedRegistration`、冲突诊断和 immutable `CapabilitySnapshot` 已交付。旧 manager registration facade 与平行 API/Admin/CLI registry 已在后续清理切片删除，不再提供双 source of truth 回滚路径。

### Slice 2：Lua loader 接入 staged activation

**目标：** 消除 Lua init 期间直接污染全局状态的问题。

1. 扩充 Lua capability snapshot，使 API/Admin/CLI contract 都包含 handler key、policy、assets 等运行字段。
2. `LuaPlugin::init` 重构为 `activate(instance_context)`：执行 Lua、收集 contract、校验 permission/policy、向 staged registrar 写入。
3. 旧 Lua surface 语法仅作为 adapter 写入 `__contract_registry`；不保留 `__pending_*` 注册表或第二条注册路径。
4. VM 在 registry commit 成功后才发布；失败时直接 drop VM。
5. `PluginManager` handler map 转为 snapshot binding + runtime instance lookup；使用 generation/`Arc` 保证在途调用安全。
6. 为 legacy Lua API 发出可诊断 deprecation event，不在本切片删除兼容路径。

**测试：** Lua init 中途失败无 route/VM 残留；重复命令冲突无部分注册；contract 与 legacy registration 结果一致。

**实施状态：完成。** Lua activation 先构造独立 VM 与 `__contract_registry`，校验并提交 staged capability 后才发布 VM，再启动 deferred task。失败 activation 不泄漏 capability、policy binding、VM 或 task；旧 Lua surface 语法仅作为写入同一 contract registry 的 adapter，并按实际使用的 API 产生一次带 plugin/API 名称的 deprecation warning。

### Slice 3：Template/static/event/task owner 化

**目标：** 使插件非 handler effect 同样可以撤销。

1. `TemplateService` 改为动态 root provider；注册/撤销由 owner 控制。
2. 插件 static root 从启动时 Axum `nest_service` 改为 registry-backed dynamic serving。
3. EventBus listener 返回 registration/disposer，并记录 owner；Lua event handler 与 Rust listener 走统一生命周期。
4. 增加 background task registrar 和 cancellation token；禁止插件直接无跟踪 `tokio::spawn`。
5. `PluginHandle` 汇总 runtime instance、registrations、subscriptions、tasks 和 drain 状态。

**测试：** remove owner 后模板、静态文件、event listener 和 task 都不可见；路径穿越仍被拒绝；取消超时被审计。

**实施状态：完成。** Template/static/event 均进入 owner-scoped capability snapshot。Lua 与 Rust builtin task 都使用 deferred `TaskRegistry`：activation/factory 成功前不启动，disable/host shutdown 按 owner 取消，successful reload 按上一代 registration IDs 定向取消；取消超时会 warning 并 abort。

### Slice 4：RuntimeHost 与真正 activate/deactivate

**目标：** 将治理开关从“调用时 guard”升级为完整生命周期。

1. 新增 `RuntimeHost::activate/deactivate/reload`，并将状态变化串行化到 instance lock。
2. `set_plugin_enabled` 改委托 RuntimeHost：持久化目标状态、执行生命周期、再返回最终 state。
3. 明确失败语义：
   - disable drain 失败：能力已从新 snapshot 移除，状态记为 inactive-with-error。
   - enable 激活失败：保持 enabled intent，但 `loaded=false`，记录 failure reason；是否自动重试由后续机制决定。
5. required plugin toggle 返回稳定错误码。

**测试：** optional 插件 disable/enable 无重启恢复；并发请求持有旧 generation 完成；新请求立即 404/command not found；治理 event 完整。

**实施状态：完成。** `RuntimeHost` 在 per-plugin runtime lock 内串行 activate/deactivate/reload；optional toggle 执行真实 VM/capability/task 生命周期，required toggle 返回 `required_plugin_toggle_forbidden`。Failed reload 保留旧 snapshot/runtime/task，successful reload 原子切换 generation 后撤销旧 effect。

### Slice 5：Profile、bundle 与 inspect

**目标：** 将产品装配从条件分支变成确定性配置。

1. 实现 schema v1 parser、bundle resolver、full-entry overlay、path resolution 和 deterministic dump。
2. 新建 `base/api/admin/official` bundles 与 `default/api/admin/minimal` profiles。
3. 默认 profile 精确复现当前产品；默认文件缺失时 fail closed，不提供目录扫描 adapter。
4. `SushiConfig` 增加 runtime/profile 配置；保留现有字段并提供兼容映射。
5. 产品组合只通过全局 `--profile` 选择；旧 surface flags 在清理阶段删除。
6. 增加 `inspect profile` 和 `inspect capabilities` bootstrap-safe 命令。

**测试：** overlay 替换、重复 ID、未知 source、required/disabled、相对路径、dump snapshot；四个 profile 的 capability golden test。

**回滚：** profile 文件错误不修改数据库或激活插件；不允许静默恢复目录扫描。

**实施状态：完成。** schema v1、bundle 顺序、full-entry overlay、路径限制、四个 shipped profile 和 stable JSON dump 均已交付。`inspect profile` 保持 bootstrap-safe，`inspect capabilities` 输出 key、owner 与 `source=builtin|rust|lua`；缺失默认 profile fail closed。

### Slice 6：统一 HTTP/Admin dispatcher

**目标：** 消除 Rust 内置路由与 Lua 插件路由的双轨安全语义。

1. 定义规范化 HTTP request/response、Rust `HttpHandler` 和 Lua handler adapter。
2. 实现 exact + trailing wildcard matcher、reserved route 检查和 deterministic conflict diagnostics。
3. Host Router 增加统一 fallback；先让 Lua plugin routes 走 fallback，保留 Rust static routes作为 shadow comparison。
4. 增加双跑/断言模式：测试环境比较旧 route map 和 snapshot map，发现差异立即失败。
5. 将 users/auth API 逐条转为 builtin route registration；每迁一组删除对应静态 nest。
6. Admin plugin page、workspace partial、插件静态资源逐步切到同一 dispatcher。
7. 当所有 route 迁移完成，删除 `build_plugin_api_routes` 和重复鉴权逻辑。

**测试：** Rust/Lua public、authenticated、policy deny/allow、cookie/bearer、body limit、wildcard、binary body、status envelope 完全一致。

**实施状态：完成。** API 与 Admin 使用稳定 Axum transport shell 和 snapshot-backed matcher/dispatcher；surface 是否挂载由 builtin 发布的 owner-scoped `transport:api/admin` capability 决定，`serve.rs` 不读取 `identity`、`api-core` 或 `host-admin` key。Rust builtin 与 Lua handler 共用规范化 request/response、authentication、policy、body limit 与错误映射。旧动态 route builder 和重复鉴权路径已删除，reserved host route 与 wildcard 冲突由 registry fail closed。

### Slice 7：首方 Rust 插件与低风险功能迁移

**目标：** 证明内置功能与 Lua 插件遵循同一 contract，而不先碰认证关键路径。

推荐顺序：

1. Admin logs 页面/API。
2. Admin config 页面/API 与 CLI config。
3. Dashboard 页面。
4. Menu contribution contract；菜单数据 CRUD 本身随后迁移。
5. Plugin workspace/read-only inspection 页面。

实现方式：

- 先在现有 `sushi-admin` / `sushi-cli` crate 内定义 builtin factory，避免早期 crate 拆分。
- profile 用 `builtin:<key>` 选择 factory。
- 路由、页面、命令、policy、assets 都由插件 activation 注册。
- 插件治理写操作属于 required `builtin:governance`，不能被普通 toggle 禁用。

**完成门槛：** 删除对应 router/main 的硬编码条目后，默认行为和权限测试不变。

**实施状态：完成。** Logs、config、dashboard、plugin workspace、menu contribution 与 Host CLI command 均在现有 crate 内通过 builtin factory activation 注册；profile 用 `builtin:<key>` 组合，不按页面拆 crate。插件治理写能力由 required `builtin:governance` 持有。

### Slice 8：Plugin migration runner

**目标：** 停止向 `sushi-cli/src/app.rs` 追加产品 migration 常量。

1. 新增 migration catalog 表和 transaction API。
2. 为历史 `001-008` 建立 owner mapping 和 existing DB bridge；不修改历史文件。
3. Runtime resolve profile 后先汇总 required migration，按依赖/稳定 ID 执行，再 activate。
4. builtin plugin 提供静态 migration descriptor；Lua plugin 读取 `migrations/*.sql`。
5. 新增 migration 只能放在 owner plugin 中；中央 `migrations/` 仅保留平台/历史基线。
6. 处理现有 KV/CMS 历史：existing DB 只 bridge；fresh DB 通过 catalog 执行相同 SQL，并用后续 forward migration 清理旧菜单 seed 的跨插件耦合。
7. checksum mismatch、重复 ID、未授权 DB migration 均 fail closed。

**数据库门禁：** 实施前备份本地 DB；先在临时 DB 和历史 DB 副本验证。此切片一旦在生产数据上执行，代码回滚不代表 schema 回滚。

**实施状态：完成。** Migration catalog 记录 owner/ID/checksum/applied time，SQL 与记录同事务。`host-core`、`policy`、`menu-admin` factory 各自提供历史 descriptor；bootstrap 按 enabled profile factory 汇总，不再有 `include_menu_admin` 产品分支。官方 Lua migration 需要 official source、manifest write/admin、`approved = true` 和 profile write/admin grant。

### Slice 9：Auth/RBAC/Admin shell/API core 插件化

**目标：** 将核心产品能力移出 Host Router，但保持其 required 和高信任属性。

1. `builtin:identity`：JWT、login/refresh/me、用户 repository capability。
2. `builtin:policy`：Authorizer、policy declaration/binding、snapshot refresh。
3. `builtin:admin-shell`：login shell、dashboard shell、workspace、favicon/global assets。
4. `builtin:rbac-admin`：users/roles/permissions 页面与 CRUD。
5. `builtin:api-core`：users/auth API route contributions。
6. `builtin:menu-admin`：menu CRUD 与 runtime contribution projection。
7. 将 `SushiContext` 拆成可信 host services 与插件可见 `PluginContext`，删除插件直接拿完整 context 的路径。

**安全门槛：** 每迁一个 required 插件都增加 profile 缺失/禁用启动失败测试和 recovery inspect 测试。

**实施状态：完成。** `policy`、`identity`、`api-core`、`admin-shell`、`rbac-admin`、`menu-admin`、`governance` 已拆为 required builtin entry；`PluginContext` 已限制插件可见 host capability。公开的 `LuaPlugin::init(&SushiContext)` 与 `inject_sushi_api` 兼容入口均已删除，测试通过私有 helper 构造所需上下文。

### Slice 10：动态 CLI 根命令

**目标：** 移除 `main.rs` 的业务 `Commands` enum。

1. 实现 bootstrap/global parser 和 runtime command tree builder。
2. 由 `builtin:host-cli` factory 发布 `serve`、`seed`、`plugin`、`config` 等 builtin command specs；Lua commands 保持 trailing args 兼容。
3. 动态 help 显示 owner、description，并按 policy/启用状态过滤或标注。
4. 统一 command authorization 和错误码，不再要求用户通过固定 `run <plugin_name>` 间接调用。
5. 在兼容清理阶段删除 `sushi run <command> -- ...` alias，根命令成为唯一业务命令入口。
6. Host 保留 `inspect`、`doctor` 和最小 recovery 命令，保证 profile/plugin 故障时仍能诊断。

**实施状态：完成。** `sushi` 单一 launcher 已以 `CliCommandSpec` 构建 Rust/Lua 统一命令树，Rust/Lua handler 共用 dispatch 和 authorization，旧 `run` alias 已删除；`host.cli` entry 控制 builtin command capability，`--version` 不 bootstrap，`--overlay-file` 按顺序接入 profile resolution。

### Slice 11：信任模型与 manifest 去产品化

**目标：** 收敛官方插件自动全权限和 core manifest 泄漏。

1. 将 source trust 从 manifest 自报 `kind` 移到 host-managed trust store/profile source policy。
2. 有效 grant = host trust ceiling ∩ manifest request ∩ profile grant ∩ administrator approval。
3. 删除 `PluginKind::Official => full permissions` 自动升级行为，并提供现有官方插件迁移配置。
4. 将 `[file_browser]` 从通用 `PluginManifest` 移到该插件的 profile entry config；core 只保留 opaque plugin config。
5. manifest schema 加版本号；只接受当前 schema v1，缺失、旧版和未来版本均提供明确升级错误并 fail closed。
6. 更新 third-party 示例和插件作者文档。

**实施状态：完成。** trust 由 source/path 决定，不再读取 manifest 自报 `kind`；显式 `approved = true` 是执行 Lua entrypoint 的前置条件，未批准 optional entry 不执行 route/command/Admin/event/task/auth/log/database 等任何 effect，required 未批准 entry 在数据库打开和 migration 前失败；批准后的有效 transport/database 权限再按 host ceiling、manifest request 和 profile grants 取交集。File-browser 产品配置只来自 profile entry config；manifest 严格要求 schema v1，schema 0、缺失版本和未来版本均 fail closed。Shipped manifest、third-party 示例和作者文档均已迁移。

### Slice 12：清理与可选 crate 物理拆分

**目标：** 在 contract 稳定后删除兼容层并改善依赖导航。

1. 删除 legacy pending registration、旧 handler maps、旧 registry 类型和双路由 builder。
2. 缩小 `PluginManager`，最终由 `RuntimeHost`、`CapabilityRegistry`、`PluginRepository` 分担职责。
3. 根据稳定依赖边界决定是否拆出：
   - `sushi-plugin-sdk`
   - `sushi-runtime`
   - `sushi-plugin-lua`
   - `sushi-host-axum`
   - `sushi-host-cli`
4. 若拆 crate，单独提交纯移动/依赖变化，不与行为改动混合。
5. 删除已结束兼容窗口的 deprecated flags/commands，并用回归测试锁定唯一入口。

**实施状态：完成（crate 拆分除外）。** snapshot 已是 API/Admin/CLI handler 的唯一 dispatch source，旧 pending table、manager 注册 facade、legacy registry/source、隐式 discovery 和旧命令/flag 均已删除，graceful shutdown 也已交付。`PluginManager` 不再公开 lifecycle toggle，插件元数据、required 集合和状态 repository 由 `PluginRepository` 持有，lifecycle lock 由 `RuntimeHost` 持有；`PluginId` 已接入 migration、状态和 template/static resource spec。runtime/Lua/host crate 的物理拆分仍是可选导航优化，不作为完成条件。

## 9. 验证策略

### 9.1 每切片最小验证

- 修改 registry/lifecycle：运行对应 `sushi-core` unit/integration test。
- 修改 Lua contract：运行 `lua_contract_kernel`、`lua_contract_registry` 和三个官方插件行为测试。
- 修改 HTTP/Admin：运行目标 router test，再运行 `cargo test -p sushi-admin --test admin_web -q`。
- 修改 CLI：运行 command parser/authorization tests，并执行 `cargo run -p sushi -- --help` 与目标动态命令 smoke test。
- 修改 migration：对 fresh in-memory/file DB、历史 DB fixture 和 checksum mismatch fixture 分别验证。

### 9.2 里程碑验证

在 Slice 4、6、8、10、12 后执行：

```bash
cargo fmt --all -- --check
cargo test -p sushi-core --test template_service -q
cargo test -p sushi-admin --test admin_web -q
cargo test --workspace -q
```

### 9.3 运行时 smoke matrix

| Profile | API | Admin | CLI dynamic | Official Lua |
|---|---:|---:|---:|---:|
| `minimal` | health only | no | host builtin + inspect/doctor | no |
| `api` | yes | no | yes | configured |
| `admin` | required support API only | yes | yes | configured |
| `default` | yes | yes | yes | cms/kv/file-browser |

每个 profile 需要验证启动 capability dump、关键 200/401/403/404、disable/enable 和 graceful shutdown。

**实施状态：完成。** 自动矩阵覆盖四 profile 的 `/health`、API surface `401/403/404`、Admin `200/404`、API-only public plugin 页面及全局/插件静态资源、default optional plugin HTTP disable/enable，以及 Axum shutdown 等待在途请求完成后再清理 owner task。Capability golden 同时锁定 `transport:api/admin` 的 owner/source。

### 9.4 最终验证记录（2026-08-19）

2026-08-19 缺口闭环后的精确目标测试与最终全量门禁均已通过：

- `cargo fmt --all -- --check`：通过。
- `cargo test -p sushi-core --test template_service -q`：8 passed。
- `cargo test -p sushi-admin --test admin_web -q`：84 passed。
- `cargo test -p sushi --test cli_baseline -q`：7 passed，覆盖旧 surface flag 拒绝、bootstrap-safe doctor、capability owner/source 诊断和 SIGTERM graceful exit。
- `cargo test --workspace -q`：通过，共 412 passed，所有 workspace 测试 target 成功退出。
- `cargo test -p sushi-core --test runtime_migrations -q`：9 passed；factory-owned builtin catalog、历史 bridge、checksum 与显式 migration grant 均通过。
- `cargo test -p sushi-core runtime::task -q` 与 successful reload 回归：通过，覆盖 owner 全量取消、registration 定向取消与 generation replacement。
- Approval、transport、legacy deprecation、四 profile HTTP matrix、optional lifecycle 与在途 drain 的精确目标测试：全部通过。
- `cargo run -q -p sushi -- --help`、`doctor`、`inspect profile`、`--profile minimal --help`：通过；默认与 minimal capability 投影符合 profile。
- `cargo run -q -p sushi -- run`：按预期以 unknown subcommand 失败，确认 alias 已删除。
- `git diff --check`：通过。
- Agent Note validator：`valid: .mozi/notes`。

### 9.5 代码审查修复记录（2026-08-20）

- `cargo fmt --all -- --check`：通过。
- `cargo test -p sushi-cli --lib -q`：25 passed；覆盖未批准 optional plugin migration 不执行与四 profile 静态资源 smoke。
- `cargo test -p sushi-core --test template_service -q`：9 passed；覆盖插件模板符号链接不能逃逸注册根。
- `cargo test -p sushi-admin --test admin_web -q`：85 passed；覆盖插件静态资源符号链接不能逃逸注册根。
- `cargo test -p sushi --test cli_baseline -q`：7 passed；覆盖单次 `serve` 只激活一次 runtime entry，以及根 help 使用临时配置和数据库。
- `cargo test --workspace -q`：通过，共 415 passed，所有 workspace 测试 target 成功退出。
- `git diff --check`：通过。
- Agent Note validator：`valid: .mozi/notes`。

### 9.6 缺口闭环最终验证（2026-08-20）

- `cargo test -p sushi-core --lib -q`：173 passed；覆盖 repository 委托、完整生命周期和状态转换。
- `cargo test -p sushi-cli --lib -q`：26 passed；覆盖 `host.cli` gating、bootstrap-safe version、有序 overlay 与 migration failure 状态。
- `cargo test -p sushi-api --lib -q`：28 passed；disable 后 owner capability 从新 snapshot 撤销。
- `cargo test -p sushi-admin --test admin_web -q`：85 passed；Admin 页面和 workspace 在 disable 后返回 `404`。
- `cargo test -p sushi --test cli_baseline -q`：10 passed。
- `cargo test -p sushi-core --test runtime_migrations -q`：10 passed；`PluginId` 无效输入 fail closed。
- `cargo test -p sushi-core --test runtime_registry -q`：22 passed；template/static resource spec 使用强类型 `PluginId`。
- `cargo test --workspace -q`、`cargo fmt --all -- --check`、`git diff --check` 和 Agent Note validator：通过。

### 9.7 文档收口复核（2026-08-20）

- 再次运行 `cargo test --workspace -q`、`cargo fmt --all -- --check`、`git diff --check` 和 Agent Note validator：全部通过。
- `runtime_profile` 11 passed、`runtime_registry` 22 passed、`runtime_migrations` 10 passed、`template_service` 9 passed、CLI baseline 10 passed。
- `default`、`api`、`admin`、`minimal` 的 `inspect profile` 与根帮助 smoke 均通过；确认 `minimal` 继承 `host.cli` 并只排除 API、Admin 和官方 Lua 产品能力。
- 权威工件、补充闭环计划和 implemented Agent Note 已统一 `inspect profile` / `inspect capabilities` 职责与 `HostCliFactory` 实际路径。

## 10. 风险与缓解

### 10.1 在途请求与卸载竞争

- **风险：** snapshot 已删除 owner，但旧 handler 引用的 Lua VM/task 被提前释放。
- **缓解：** snapshot binding 持有 runtime instance `Arc`；dispose 先发布新 snapshot，再 drain，最后释放 owner handle。

### 10.2 自禁用死锁

- **风险：** 插件 handler 内触发自身 disable，与 instance runtime lock 相互等待。
- **缓解：** `RuntimeHost` 与治理入口在 per-plugin runtime lock 内串行 mutation；handler dispatch 不持有该 write lock，回归测试覆盖 optional toggle 与 reload。

### 10.3 路由优先级变化

- **风险：** Axum 原生 route 与新 matcher 对 exact/wildcard 的优先级不同。
- **缓解：** Registry 对 exact/wildcard 与 reserved route 做确定性冲突检查；统一 dispatcher 的路由、鉴权、body、binary response 和 404 行为由 API/Admin golden tests 锁定。

### 10.4 required 插件导致启动不可恢复

- **风险：** auth/policy/governance profile 配错后普通 CLI 也无法启动。
- **缓解：** launcher 保留不依赖业务 runtime 的 inspect/doctor；required entry 缺失时输出完整来源和修复建议。

### 10.5 Migration 不可逆

- **风险：** plugin owner/bridge 映射错误会影响已有数据或阻塞启动。
- **缓解：** 历史 SQL 不改；先只读 bridge audit，再写 catalog；所有实施先跑 DB 副本，生产前备份。

### 10.6 兼容层重新引入

- **风险：** 后续功能绕过 staged registry，重新引入 pending table、隐式 profile discovery 或 manager facade。
- **缓解：** 生产注册只暴露 owner-scoped staging；manifest/profile/CLI 回归测试锁定 fail-closed 与单通路行为。

### 10.7 首方插件过度拆分

- **风险：** 为追求“皆插件”制造大量细碎 crate 和循环依赖。
- **缓解：** 先按生命周期和替换边界拆逻辑 plugin，不按页面拆 crate；crate 拆分晚于 contract 稳定。

### 10.8 权限模型变化

- **风险：** 删除 official 自动全权限后导致官方插件启动失败，或兼容过宽继续留下提权入口。
- **缓解：** shipped profile 显式写入 `approved = true` 与所需 grant；缺失审批不执行插件入口或任何 effect，required 缺失审批在打开数据库前失败，manifest/profile/grant、migration 与 effect 测试锁定 fail-closed 行为。

## 11. 回滚策略

- 兼容 facade 与 `legacy-default` 已删除；回滚必须显式恢复 profile 文件和对应代码版本，不允许在运行时静默扫描并加载全部插件。
- HTTP/Admin dispatcher 已完成单通路迁移；回滚只能显式恢复对应代码版本，不能在运行时切回旧 route map。
- Migration slice 之后只做代码前滚；若必须回退二进制，旧版本需确认能容忍新增表/列。不能依赖自动 down migration。
- Trust enforcement 已是 fail-closed；回滚前必须确保目标版本理解 schema v1 manifest、profile grant 与新增 migration catalog。
- 本轮未执行提交；工作区保留完整变更供后续审查和按逻辑拆分提交。

## 12. 实施顺序摘要

```text
基线/ADR
  -> owner registry
  -> Lua staged activation
  -> template/static/event/task ownership
  -> activate/deactivate
  -> profile/bundle
  -> unified HTTP/Admin dispatch
  -> low-risk builtin plugins
  -> plugin migrations
  -> auth/RBAC/admin shell plugins
  -> dynamic CLI
  -> trust + manifest cleanup
  -> compatibility removal / optional crate split
```

## 13. 已批准架构取舍

用户已于 2026-08-18 批准以下全部架构取舍：

1. 接受“稳定 Host Router + snapshot fallback dispatcher”，而不是热交换完整 Axum Router。
2. 接受首方 Rust 插件第一版静态链接，不建立动态库 ABI。
3. 接受 profile overlay 按 entry ID 完整替换，不使用 deep merge。
4. 接受 required 插件不能通过普通治理接口禁用，并保留 bootstrap-safe recovery 命令。
5. 接受 migration forward-only；历史 `001-008` 通过 bridge 保持不变。

实施已按 Slice 0-12 的行为范围完成；可选 crate 物理拆分未纳入本轮，也未与行为迁移混入同一变更。
