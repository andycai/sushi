# AGENTS.md — 已实施 Agent Note

本文件只对 `.mozi/notes/implemented/` 生效，并继承[仓库根规则](../../../AGENTS.md)和 [Notes 根规则](../AGENTS.md)。[README.md](../README.md) 仍是格式与生命周期合同的唯一权威规范。这些 Agent Note 记录已经交付的决策；使用 `mozi-agent-notes/scripts/validate_agent_notes.py` 检查是否符合 implemented 生命周期的结构要求。

## 让已实施 Agent Note 与实际交付保持一致

当路径、符号、默认值或实现机制发生变化时，必须在同一个变更中同步更新 Agent Note。直接在原文中改写过时事实；不要在文末追加变更历史。

当一份已交付的记录不太可能再为后续工作提供指导时，应通过技能 `mozi-agent-notes` 归档 Note 文档，而不是继续维护它。

### 这不意味着可以改写原有的*决策*

只在原记录中更新决策的事实性实现。如果决策本身或决策依据发生逆转，必须新建一份 Agent Note 并建立交叉链接；只有在遵循 [Agent Note 规则](../README.md) 中的合并规则时，才可以删除被完全取代的旧记录。
