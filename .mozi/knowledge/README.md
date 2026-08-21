# 项目知识管理合同

本目录统一管理项目的持久知识。核心原则是：**一条知识只有一个权威去向**。其他文档可以引用权威内容，但不得复制并维护第二套格式、生命周期或事实。

## 知识路由

| 知识类型 | 权威去向 | 直接入口 | 适用内容 |
|---|---|---|---|
| 稳定规则 | `rules/GENERAL.md`、`rules/PROJECT.md` | 当前任务直接更新 | 跨项目行为准则、当前项目稳定事实与约定 |
| 模块事实 | 模块旁的 `CONTEXT.md` | `mozi-context` | 模块接口、依赖、使用方式和局部经验 |
| 设计决策 | `decisions/` | `mozi-agent-notes` | 决策理由、真实备选方案、权衡和长期约束 |
| 纠错经验 | `lessons/` | `mozi-reflect` | 实际错误中提炼、尚未被稳定规则或机器约束吸收的做法 |
| 会话状态 | `.mozi/work/handoffs/` | `mozi-handoff` | 当前进度、恢复工作所需的临时上下文 |
| 来源型知识 | 显式初始化的 LLM Wiki | `mozi-llm-wiki` | 外部资料、编译后的领域知识和可追溯来源 |

计划、调研和展示页面属于工作产物，写入 `.mozi/work/artifacts/`；循环工作流状态写入 `.mozi/work/loops/`。它们可以提供证据或上下文，但不是稳定规则、模块事实或设计决策的权威副本。

## 冲突优先级

出现冲突时按以下顺序判断当前行为：

1. 用户在当前任务中的明确指令。
2. 可执行代码、测试和确定性验证器。
3. 目录作用域内适用的 `AGENTS.md`。
4. `rules/GENERAL.md` 与 `rules/PROJECT.md`。
5. 模块旁的 `CONTEXT.md`。
6. 活跃 Agent Note。
7. `lessons/` 中的活跃教训。
8. handoff、artifact、LLM Wiki 和归档内容。

低优先级内容与高优先级事实冲突时，应更新或归档低优先级内容，而不是用多个副本解释差异。

## 加载合同

代理初始化时依次读取：

1. 本文件；
2. `rules/GENERAL.md`；
3. `rules/PROJECT.md`；
4. `LESSONS.md`；
5. `lessons/` 下全部活跃分类文件。

`lessons-archive/`、Agent Note、handoff、artifact 和 LLM Wiki 按任务需要读取，不进入默认启动上下文。

## 纠错与提升

`mozi-reflect` 只处理实际发生的纠错事件，不是所有知识写入的万能入口。普通模块知识直接使用 `mozi-context`，设计决策直接使用 `mozi-agent-notes`。

首次发生且未来可能复现的错误，写入合适的活跃 lesson。重复犯错时先诊断原因：未加载、未命中、未执行、放错位置、无法靠提示保证或规则已过时；然后选择更新 lesson、提升为稳定规则或模块事实、补充机器约束、归档过时内容，或确认无需写入。

lesson 被以下任一权威机制完整吸收后，应移入 `lessons-archive/` 并写明 `归档：` 原因：

- 成为 `GENERAL.md` 或 `PROJECT.md` 的稳定规则；
- 成为模块 `CONTEXT.md` 的局部事实；
- 成为 Agent Note 管理的设计决策；
- 已由代码、测试、验证器或安全门禁可靠保证；
- 所依赖的机制已经退出当前项目。

归档知识不是当前行为依据。需要重新启用时，应根据当前事实重写为新的活跃条目，而不是直接复制过时文字。

## 维护边界

- `LESSONS.md` 只定义 lesson 分类、格式、容量和归档合同。
- `decisions/README.md` 只定义 Agent Note 的路径、格式和生命周期。
- `AGENTS.md` 只保留启动顺序、硬约束和入口链接，不复制本文件细节。
- handoff 只保存恢复会话所需状态，不设置 lesson 专门章节。
- 历史 artifact 和 deprecated 内容保留其时代语境，不作为当前路径扫描的修复对象。
