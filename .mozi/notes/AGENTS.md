# AGENTS.md — Agent Note（代理记录）

本文件对整个 `.mozi/notes/` 目录树生效，并继承[仓库根 `AGENTS.md`](../../AGENTS.md)。Agent Note 本质上是由代理编写的 RFC：用于长期保留提案和决策记录，说明决策依据、曾考虑的替代方案、后果以及所需的验证。[README.md](README.md) 是路径、生命周期、分类和文件格式的唯一权威规范；本文件只规定代理在 Notes 树中的通用操作。

**每新增一份 Agent Note，都必须检查是否取代了旧记录。** 在活跃目录树中搜索涉及相同决策或机制的旧记录，使用唯一入口技能 `mozi-agent-notes` 判断是否属于完全取代或部分取代。部分取代的记录应继续保持活跃，并与新记录互相链接；完全取代时先保留旧记录的独有理由和证据，再按授权规则处理旧文件。合同为 Markdown-only，不创建双语对侧文件、manifest 或 sidecar。

[`archived/`](archived/AGENTS.md) 下的文件是冻结的历史快照：绝对不要编辑它们，也不要把它们当作当前行为的权威依据。

进入 `implemented/` 或 `archived/` 工作时，还必须读取对应目录的局部 `AGENTS.md`。当前没有 `proposed/AGENTS.md` 或 `rejected/AGENTS.md`，这两个生命周期只继承本文件和 README 合同。
