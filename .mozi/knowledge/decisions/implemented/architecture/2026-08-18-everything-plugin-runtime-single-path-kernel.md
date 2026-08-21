# Agent Note: 建立一切皆插件的单通路运行时内核

Status: implemented

## 问题

Sushi 曾同时存在宿主硬编码产品能力、Lua pending table、`PluginManager` 注册 facade、独立 surface registry 和隐式默认插件发现。注册可能绕过 owner 生命周期，产品组合也可能由目录内容或兼容命令隐式改变，导致激活失败回滚、停用撤销、权限审计和 Rust/Lua 对称性无法由一个内核合同保证。

## 决策

所有 Rust 与 Lua transport capability 都通过带 `PluginInstanceId` owner 的 staged registry 注册，校验成功后原子发布到 immutable snapshot。API/Admin transport surface、HTTP、CLI、template、static 和 event 共享该 snapshot；`identity` 与 `admin-shell` 分别发布 `transport:api` / `transport:admin`，`serve.rs` 不检查产品 builtin key。后台 task 使用独立的 owner-scoped `TaskRegistry`；Lua runtime handle 保存 generation registration IDs，Rust builtin factory 通过 owner-scoped `PluginContext` 暂存 task，只有 factory 成功后才启动。停用或进程退出按 owner/generation 撤销 registration、subscription、task 和 VM。`PluginRepository` 持有插件元数据、required 集合和 `PluginStateRepository`，`PluginManager` 只保留 VM dispatch 与 capability facade；按插件生命周期串行化的 lock 由 `RuntimeHost` 持有。

旧 `sushi.api`、`sushi.cli`、`sushi.admin`、`sushi.web` 与 `sushi.event.on` Lua 语法只作为 contract adapter 保留，直接写入 `__contract_registry`，不维护 pending table 或第二套注册状态。每种实际使用的 direct registration adapter 会产生一条带 plugin/API 名称的 deprecation warning。生产代码不再暴露 `PluginManager::register_*` facade、独立 API/Admin/CLI registry 或 `RegistrationSource::Legacy`。

产品组合只来自显式 profile、bundle 和 launch overlay。默认 profile 缺失时 fail closed，不扫描插件目录生成 `legacy-default`。Required builtin 与 Lua plugin 使用同一 capability activation 模型；required entry 只能通过 profile 变更和受控重启调整。`doctor` 与 `inspect profile` 保持 bootstrap-safe；`inspect profile` 只输出最终 entry/config/grants/origin，`inspect capabilities` 在启动所选 runtime 后追加 active capability key、owner 与 registration source。`doctor` 不执行插件入口，并对 manifest、approval、migration checksum、legacy bridge 与 forward recovery 做只读检查，诊断携带 entry/source 和修复建议。

Rust builtin 由静态 `BuiltinFactoryRegistry` 按 profile source key 发现；factory 同时提供 activation 和 owner-local migration descriptor。`host-core`、`policy` 与 `menu-admin` 各自拥有其历史 migration，bootstrap 不再用产品 surface 布尔分支汇总 builtin schema。

Manifest 严格要求 `schema_version = 1`。信任上限由 host 根据 source/path 决定，manifest 只声明权限请求，profile 提供 grant；`approved = true` 是执行 enabled Lua entrypoint 与纳入 Lua migration catalog 的共同前置条件。未批准 optional entry 不执行 route、command、Admin、event、task、auth、log、migration 或 database effect，required 未批准 entry 在打开数据库和 migration 前失败。Manifest 自报 `kind`、通用 `[file_browser]` 产品配置和 schema 0 reader 均不存在。migration descriptor、插件状态和 template/static resource spec 的核心 package identity 使用 `PluginId`，SQL/JSON 展示边界显式转换为字符串。动态 CLI 根命令是唯一业务命令入口，所有 builtin/Lua 业务命令共用 policy authorization；`crates/sushi-cli/src/builtin.rs` 中的 `HostCliFactory` 由 `host.cli` profile entry 激活并发布 `serve`、`plugin`、`config` 与 `seed`。launcher bootstrap 一次并把同一个 `SushiContext` 交给这些 handler 与 `inspect capabilities`，命令结束后统一清理 owner task。缺少 `host.cli` 的 profile 不发布 Host 业务命令；shipped `minimal` 继承 `base` 中的 required `host.cli`，因此保留这些命令，但不激活 API、Admin 或官方 Lua 产品能力。`sushi run`、`serve --api-only` 和 `serve --admin-only` 均已删除。

静态资源是 Host transport，不由 Admin transport 独占。Host Router 始终挂载全局静态目录、favicon 与动态插件静态 handler，因此 API-only profile 的公共插件页面也能加载所需 CSS/JS。动态插件静态文件与模板文件在读取前同时规范化注册根和目标路径，目标必须仍位于规范化根内且是普通文件；目录内指向根外的符号链接按资源不存在处理。

## 所有权与生命周期

`RuntimeHost` 串行化 activate、deactivate 和 reload，并暴露 `Discovered`、`Migrating`、`Resolved`、`Activating`、`Active`、`Deactivating`、`Inactive` 与 `Failed` 状态。bootstrap 在解析 Lua source 后进入 `Discovered`，构建和执行 migration catalog 前进入 `Migrating`，迁移失败保持 `Failed` 且不会进入激活；成功迁移后进入 `Resolved`。激活期间 runtime instance、capability、subscription 与 task 先进入 owner-scoped staging；任何冲突、权限错误、Lua 初始化错误或 manifest/profile 错误都不会发布部分状态。停用先让新 snapshot 不再暴露 owner capability，再取消并 drain 对应 generation，旧 snapshot 的在途调用通过持有的 runtime instance 引用完成。Successful reload 发布新 generation 后按上一代 task registration IDs 定向取消旧 task；failed reload 保留旧 snapshot、VM 和 task。

`sushi serve` 在 Ctrl-C 或 Unix SIGTERM 后让 Axum 停止接收新连接并 drain 在途请求，随后取消所有 owner task；超时未合作的 task 会被 abort。`inspect capabilities` 输出稳定 capability key、owner 和 registration source，不暴露运行期自增 registration ID。

Host 仅保留配置解析、身份与权限强制、生命周期、审计、迁移、运行时隔离和 transport dispatch 等可信边界。产品 API、Admin 页面和动态 CLI 命令由 profile 选择的 builtin/Lua plugin contribution 提供。

## 曾考虑的替代方案

**保留多套 registry 并通过同步代码维持一致。** 该方案继续允许 owner metadata、handler map 和 pending table 分离，在部分失败和卸载时无法证明原子性，因此删除平行 source of truth。

**保留隐式默认 discovery 与 `sushi run` 兼容入口。** 该方案使磁盘目录和历史命令继续影响产品组成，削弱 profile 的唯一性和诊断确定性，因此兼容窗口结束后直接删除。

**由 manifest 自报 official/third-party trust tier。** 插件不能成为自身信任等级的权威；信任必须来自 host 管理的来源与 profile grant，因此删除 `kind` reader。

**立即拆分 runtime、Lua 与 host crates。** 物理 crate 边界不是行为正确性的前提，并会把大规模移动与生命周期迁移混在一起；本轮保持内核在现有 workspace 边界内，后续仅在依赖方向稳定且导航收益明确时单独拆分。

## 测试

Owner staging、transport surface ownership、冲突回滚、remove owner、profile composition、strict manifest schema、approval-before-execution、Lua adapter deprecation diagnostics、动态 HTTP/Admin/CLI dispatch、CLI authorization、Lua/Rust task activation/cancellation、template/static lifecycle、factory-owned plugin migration、doctor checksum/recovery、removed CLI flags 和 capability source diagnostics 均有针对性测试。四 shipped profile 自动矩阵覆盖 capability、关键 HTTP `200/401/403/404`、optional disable/enable、API-only 页面及 CSS/JS 静态资源和在途 graceful drain；profile/help smoke 额外锁定 shipped `minimal` 的 Host CLI 投影，以及无 `host.cli` 自定义 profile 不显示 Host 业务命令。额外回归测试锁定未批准 optional migration 不执行、静态与模板符号链接不能越界、单次 `serve` 只激活一次 runtime entry，以及根帮助测试只使用临时配置和临时数据库。最终验证包括 `cargo fmt --all -- --check`、`cargo test -p sushi-core --test template_service -q`、`cargo test -p sushi-admin --test admin_web -q`、`cargo test --workspace -q`、CLI smoke matrix 与 `git diff --check`。

## 相关内容

- Owner-scoped capability registry ADR: `.mozi/memories/adr/0001-everything-plugin-runtime-owner-scoped-registry.md`
- Profile composition ADR: `.mozi/memories/adr/0002-everything-plugin-runtime-profile-composition.md`
- Everything-plugin runtime implementation plan: `.mozi/artifacts/everything-plugin-runtime/2026-08-18-everything-plugin-runtime.md`

## 后果

注册、冲突检测、权限检查、运行时绑定和撤销现在共享 owner-scoped source of truth，Rust/Lua capability 与 task 具有对称的 activation 成功/失败语义，profile/capability dump 与 CLI help 能稳定解释能力来源。静态 transport 的 Host 所有权保证非 Admin profile 也能完整交付公共页面，同时 canonical containment 增加了每次资源读取的文件系统解析开销。动态 CLI 只拥有一个 runtime context，避免重复入口、task 与日志，但 launcher 必须在命令结束时移除自身 handler owner 并统一 shutdown。代价是旧 manifest、隐式 discovery、manager facade、`sushi run` 和 surface compatibility flags 不再兼容；升级方必须提供 schema v1 manifest、显式 profile、`approved = true` grant 并使用根命令。未批准插件不再获得“零 transport 权限但仍执行代码或迁移”的兼容行为。Task 作为 effect registry 而非 capability snapshot 项，需要 registration ID 维持 reload generation 边界。可选 crate 物理拆分仍被推迟，当前模块边界需要继续依赖代码约定和测试防止反向依赖。
