# 插件生命周期指南

本指南描述 Lua runtime entry 的 activation、disable、enable、reload 和进程退出语义。Builtin entry 在启动时由 profile 选择并激活；factory 接收 owner-scoped `PluginContext`，其后台 task 同样遵守成功后启动、关机时取消的 effect 生命周期。Required entry 不支持普通运行时 toggle。

## 状态

`PluginLifecycleState` 包含：

```text
Discovered → Migrating → Resolved → Activating → Active
                 │                       │             │
                 ▼                       ▼             ▼
               Failed                  Failed     Deactivating → Inactive
```

`RuntimePluginStatus.last_error` 保存最近一次 activation/reload/deactivation 错误。治理表中的 `enabled` 表示意图，`loaded` 表示当前 generation 是否已成功发布。

## Activation

Lua activation 按以下顺序执行：

1. 为 profile entry 创建带稳定 `PluginInstanceId` 的 `PluginContext`。
2. 创建独立 Lua VM，注入经过权限裁剪的 `sushi.*` API。
3. 执行入口文件和 `app.init()`/`sushi.init()`。
4. 把 route/page/command/menu/template/static/event contribution 写入 staged registrar。
5. 校验 reserved key、冲突、permission 和 policy scope。
6. 持久化 policy binding，并原子发布 capability snapshot 与 Lua VM。
7. 启动此前暂存的 owner task。
8. 创建 `PluginHandle`，记录 runtime generation、registration IDs、task IDs 和 cancellation token。

任何在 publish 前发生的错误都不会启动 task，也不会发布半套 capability 或 VM。Required entry activation 失败会阻止启动；optional entry 标记 `loaded=false` 并保留诊断错误。

Enabled Lua entry 若缺少 `grants.approved = true`，宿主不会执行入口代码，也不会给 event/task/auth/log 等非 transport effect 产生运行机会。Required 未批准条目在 migration 和数据库打开前失败。

## Disable 与 Enable

普通治理接口只允许操作 optional plugin。

Disable：

1. 持久化 `enabled=false` 意图。
2. 将状态转为 `Deactivating`，取消当前 generation。
3. 从新 snapshot 删除 owner capability，使新 dispatch 立即失效。
4. 取消 owner task；超时未退出的 task 会被 abort。
5. 删除 policy binding、Lua VM 和 handle。
6. 标记 `Inactive`、`loaded=false`。

Enable：

1. 持久化 `enabled=true` 意图。
2. 重新执行完整 activation。
3. 成功后发布新 generation 并标记 `Active`。
4. 失败时保持 enabled intent，但 `loaded=false`，便于诊断和后续重试。

Required plugin toggle 返回稳定的 `required_plugin_toggle_forbidden` 错误，必须修改 profile 并重启。

## Reload

Reload 在 plugin runtime lock 内串行执行，并保持旧 generation 直到新 activation 成功。

- **成功：** 发布新 snapshot/VM，建立只包含新 task IDs 的 handle，然后按上一代 task registration IDs 定向取消旧 task。相同 owner 的新 task 不会被误杀。
- **失败：** 保留旧 snapshot、VM、task 和 `Active` 状态，在 `last_error` 记录 reload 错误；新 activation 暂存的 task 不会启动。

Task registration ID 是 reload generation 的 effect 边界；不能仅按 owner 全量取消，因为新旧 generation 使用同一个 owner。

## 进程退出

`sushi serve` 监听 Ctrl-C，并在 Unix 上监听 SIGTERM：

1. Axum 停止接收新连接并等待在途 HTTP 请求完成。
2. `SushiContext::shutdown()` 取消所有 owner task，最多等待每个 owner 五秒。
3. 不合作的 task 被 abort，随后进程返回。

关机不会改写 optional plugin 的 `enabled` 治理意图。

## 诊断

```bash
# 不打开数据库，检查组合是否可解析
sushi inspect profile --profile default

# 完成 bootstrap 后检查 capability owner/source
sushi inspect capabilities --profile default

# 不执行插件代码，检查配置、profile、manifest、approval、checksum 与恢复条件
sushi doctor --profile default

# 查看或操作 optional plugin
sushi plugin status <name>
sushi plugin disable <name>
sushi plugin enable <name>
```

默认 profile 缺失、manifest schema 错误、未知 source、未批准 grant、migration checksum mismatch 和 required activation failure 都会 fail closed，不会回退到目录扫描。

Legacy `sushi.api.route`、`sushi.admin.page`、`sushi.cli.command` 和 `sushi.web.page` adapter 仍写入唯一 contract registry，但每种实际使用的 API 会产生一条带 plugin/API 名称的 deprecation warning。
