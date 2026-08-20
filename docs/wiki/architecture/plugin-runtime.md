# 插件运行时

Sushi 的运行时是一个 profile 驱动、owner-scoped、单 capability 通路的内核。Rust builtin 和 Lua plugin 在注册、冲突检测、诊断、dispatch 与撤销方面使用同一组稳定合同。

## 运行时拓扑

```text
profile + bundles + overlays
            │
            ▼
 RuntimeProfileResolver ── fail closed
            │
      ResolvedRuntimeProfile
            │
     ┌──────┴────────┐
     ▼               ▼
builtin source     Lua source
factory registry   manifest + grants
     │               │
     └──────┬────────┘
            ▼
       PluginContext
            │
     staged capabilities
            ▼
 immutable CapabilitySnapshot
            │
 transport:api/admin / HTTP / CLI / template / static / event dispatch

owner tasks ── deferred TaskRegistry ── cancel by generation/owner
```

## Profile 是唯一组合入口

`RuntimeProfileResolver` 解析 `profiles/<name>.toml`、有序 bundle 和 full-entry overlay，产出稳定的 `ResolvedRuntimeProfile`。

- 未指定 profile 时严格加载 `default`。
- 缺失或非法 profile 在打开数据库前失败。
- `source` 只能是已注册的 `builtin:<key>` 或受限的 `lua:<tier>/<directory>`。
- `serve` 只通过全局 `--profile` 选择 API/Admin/official 组合。
- API/Admin 是否挂载由 builtin 注册的 `transport:api` / `transport:admin` capability 决定，`serve.rs` 不读取具体 builtin key。

`inspect profile` 不打开数据库即可输出最终 entry 顺序、source、enabled/required、config、grants 和 origin。

## Builtin factory

宿主在 bootstrap 时构建 `BuiltinFactoryRegistry`。`sushi-core`、`sushi-api` 和 `sushi-admin` 分别注册自己的 `BuiltinPluginFactory`，profile resolver 只接受 registry 中存在的 key。

激活 builtin 时，bootstrap 按 resolved entry 顺序调用：

```rust
factory.activate(&ctx, &plugin_ctx, entry).await
```

Factory 接收 owner-scoped `PluginContext`，Rust builtin task 与 Lua task 一样先暂存，factory 成功后才启动。因此产品能力不再由 `serve.rs` 中的产品分支直接装配；`serve` 只根据 snapshot 中的 transport capability 构建稳定 shell。

## Owner-scoped capability snapshot

`CapabilityRegistry` 保存以下注册：

- API/Admin transport surface；
- HTTP API/Admin route；
- Admin page；
- CLI command；
- menu contribution；
- template/static root；
- event subscription。

每项注册都有 `PluginInstanceId`、`RegistrationId` 和 `RegistrationSource`。Activation 先使用 `StagedRegistrar` 收集并校验全部 contribution，再一次发布新 immutable snapshot。Dispatcher 读取 snapshot，因此 remove owner 后新请求立即看不到旧能力；已经持有旧 snapshot/runtime 引用的在途调用可以完成。

`inspect capabilities` 按 capability key 排序输出：

```text
<key>  owner=<instance-id>  source=builtin|rust|lua
```

运行期自增 registration ID 不进入稳定诊断输出。

## Task 与 generation

Task 不放进 capability snapshot，因为它是正在运行的 effect，而不是 transport contribution。Rust builtin 和 Lua plugin 都通过 `PluginContext::register_task` 把 task 暂存在 activation context；Lua 对应 `sushi.task.spawn` 和 `sushi.task.interval`。

- capability/VM commit 之前不启动 task；
- `PluginHandle` 保存本 generation 的 task registration IDs；
- disable 按 owner 取消全部 task；
- successful reload 发布新 generation 后，仅按上一代 IDs 取消旧 task；
- failed reload 不启动新 task，并保留旧 generation；
- host shutdown 在 HTTP drain 后取消所有 task。

这让 capability 与 task 虽位于不同容器，仍遵循同一个 owner/generation 生命周期。

## 启动顺序

1. 加载配置并解析 profile。
2. 加载 Lua manifest，计算 source trust、manifest request、profile grants 与 `approved = true` 的权限交集；required 未批准条目在打开数据库前失败，optional 未批准条目不执行入口代码。
3. 汇总 selected builtin/Lua migration catalog 并执行 forward-only migration。
4. 创建 `SushiContext`、capability registry、task registry、template service 和 runtime host。
5. 按 profile 顺序激活 builtin factory。
6. 注册 Lua source；required/optional 条目按 profile 与治理状态激活。
7. 刷新 policy snapshot，返回可供 API/Admin/CLI 共用的 context。

## Migration ownership

- 平台基线与 plugin governance schema 由 `builtin/host-core` 拥有。
- Policy migration 由 `builtin/policy` 拥有。
- Admin menu migration 仅在选择 `builtin/menu-admin` 的 profile 中运行。
- 官方 Lua migration 需要 official source、manifest 写权限、`approved = true` 和 profile 的 write/admin grant。
- 每项 migration 记录 owner、稳定 migration ID、checksum 与 applied time；checksum 改变时拒绝启动。

## Host 边界

Host 保留：profile/config 解析、授权强制、生命周期串行化、审计与治理、migration、Lua 隔离、stable Axum transport、dynamic CLI launcher 和 bootstrap-safe `doctor`/`inspect profile`。

`doctor` 不执行插件入口；它解析 manifest/grant，编译 migration catalog，并在数据库已存在时只读校验 checksum、legacy bridge 和 forward recovery 条件。错误包含 entry/source 和修复建议。

业务 API、Admin 页面、CLI 命令和插件 Web 资源由 profile 选择的 builtin/Lua contribution 提供。
