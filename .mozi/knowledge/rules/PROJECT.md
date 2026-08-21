# 墨子 (Mozi) — 项目约定

个人 AI 辅助编程工具箱，skills + agents + hooks + extensions 的集合项目，面向多代理环境（pi / Claude Code / Cursor）。

## 目录结构

```
mozi/
├── .agents/                # 已发布技能/代理的链接目录（gitignore，不提交）
├── .claude/                # Claude 环境的链接目录 + hooks（gitignore，不提交）
├── .mozi/
│   ├── knowledge/          # 规则、教训与 Agent Note
│   │   ├── rules/           # GENERAL.md / PROJECT.md
│   │   ├── lessons/         # 活跃纠错经验
│   │   ├── lessons-archive/ # 已归档纠错经验
│   │   └── decisions/       # Agent Note 生命周期目录
│   ├── work/               # 工作过程产物
│   │   ├── artifacts/       # PRD、计划、方案和展示页面
│   │   ├── handoffs/        # 会话交接文档
│   │   └── loops/           # 循环技能状态与日志
│   ├── config/             # 配置文件：spec-config.json
│   ├── packages/           # 源码包（按包名组织，见下）
│   └── scripts/             # 链接、CodeGraph、技能列表脚本（sh + ps1）
├── deprecated/             # 弃用内容（karpathy-*、旧 mz-* 技能、packages/mz、packages/sp 旧版包）
├── data/                   # 运行时状态（gitignore）
├── sessions/               # 会话记录（gitignore）
├── tmp/                    # 临时实验目录（gitignore）
├── .worktrees/             # git worktree（gitignore）
├── AGENTS.md               # 代理初始化入口
├── CLAUDE.md               # 指向 AGENTS.md
└── README.md
```

- **源码在 `.mozi/packages/`，发布靠符号链接**：`.agents/` 和 `.claude/` 下的内容是链接，不直接修改
- `.agents/`、`.claude/`、`tmp/`、`sessions/`、`data/`、`.worktrees/` 均被 gitignore，克隆后需重跑链接脚本

## 源码包（.mozi/packages/）

| 包 | 内容 | 状态 |
| ---- | ------ | ------ |
| `mozi/` | 当前主包：skills/（`mozi-*` 技能集合） | 当前版本，已链接发布 |
| `pi/` | pi 扩展：extensions/mozi-permission、mozi-protected-paths | TypeScript 扩展 |
| `third-party/` | 第三方技能：docx、pdf、pptx、xlsx | 外部引入 |
| `full/` | 按领域组织的专业代理定义 | 完整代理库 |
| `matt/` | 独立技能：diagnose、improve-codebase-architecture、prototype、research、teach | 实验/独立 |

## 技能约定

- 技能以 `mozi-` 前缀命名（如 `mozi-plan`、`mozi-reflect`、`mozi-commit`），通过 `/skill <name>` 调用
- 当前主包 `mozi/skills/` 按职责分三类：
  - `engineering/`：交付流水线——plan、use-worktree、implement、code-review、commit、finish
  - `knowledge/`：知识的产生、组织与检索——context、reflect、agent-notes、llm-wiki、llm-research
  - `productivity/`：多主体协作、技能编写与 HTML 内容展示——herdr、loop、handoff、grill、write-a-skill、html-presenter
- `.mozi/knowledge/decisions/` 是本仓库的长期决策记录目录；Agent Note 创建、更新、取代检查、生命周期迁移与归档统一使用 `mozi-agent-notes`，模块事实使用 `mozi-context`，实施计划使用 `mozi-plan`
- 旧 `mz-` 前缀技能与旧版包已整体移入 `deprecated/packages/mz/`，不再链接发布
- engineering 各入口在技能目录中包含完整运行合同，不依赖分类父目录文件；跨阶段字段、状态迁移（`next_action`）与 worktree 所有权令牌的一致性由 `test_engineering_contract.py` 锁定
- 交付流水线顺序：`plan? → use-worktree? → implement → verify → code-review → (fix → verify)* → commit → finish?`；plan / use-worktree 是按需动作，清晰低风险变更可直接 implement；commit 独占普通提交并需用户确认，finish 只消费已提交工作；安全、资金、数据写入或不可逆决策必须在实现前显式暴露
- 完整验证：指项目在 `CONTEXT.md / package.json / Makefile` 等处声明的完整测试与检查命令；无显式约定时使用可复现的全量测试命令。缺少可复现命令时停止并报告，不以目标测试冒充完整验证

## 文档约定

- 项目地图描述目录职责、分类和能力，不写技能、代理或分类的易漂移总数

## 代理约定

- 当前仓库内只链接发布 `code-reviewer.md`（源码在 `deprecated/packages/mz/agents/mozi/`，链接到 `.agents/agents/` 与 `.claude/agents/`）
- 完整代理库在 `full/agents/`，按领域子目录组织，供外部项目链接使用
- 代理通过 `<domain>-<role>.md` 命名（如 `engineering-backend-architect.md`）

## 规则加载

四步体系：`.mozi/knowledge/README.md` → `rules/GENERAL.md` → `rules/PROJECT.md` → `LESSONS.md` 与 `lessons/` 全部活跃分类文件

## 链接与脚本

- `link-package.sh <package> [-t DIR]` — 将 `.mozi/packages/<package>/` 的 skills/agents 链接到目标目录（默认 `.claude`；示例 `link-package.sh mozi -t .agents`）
- `list-skills.sh` — 列出所有包中的技能（按 SKILL.md 递归查找）
- `codegraph.sh` — CodeGraph 安装/索引/MCP serve
- 每个脚本均有对应 `.ps1` 版本（Windows）
- 目标 skills 目录若是指向本仓库的符号链接，脚本会报错退出，避免循环链接

## MCP

- 仓库内无 MCP 配置文件；CodeGraph 可通过 `codegraph.sh serve` 以 MCP server 方式提供代码智能服务

## 技术栈

- 技能/代理：Markdown 指令文件（SKILL.md、agent .md）
- 脚本：Bash（macOS/Linux）+ PowerShell（Windows）+ Python
- pi 扩展：TypeScript（含 package.json、index.ts、index.test.ts）
