# AGENTS.md — 已归档 Agent Note

本文件只对 `.mozi/notes/archived/` 生效，并继承[仓库根规则](../../../AGENTS.md)和 [Notes 根规则](../AGENTS.md)；格式与归档合同仍以 [`README.md`](../README.md) 为准，README 是唯一权威规范。类别目录下的已归档 Agent Note Markdown 文件是冻结的历史快照，不是当前行为的权威依据。绝对不要编辑、重新格式化、翻译、修复、删除或移动已封存的文件；新的决策和事实应记录在活跃的 Agent Note 中，或写入当前文档。

归档变更只能做以下几件事：移动 Markdown Note 文件；在 `Status: implemented` 行下方插入 `Archived: YYYY-MM-DD` 行；以及修复或删除指向这些文件的活跃入站链接。不要检查、验证或修复归档记录的出站链接。

请运行技能 `mozi-agent-notes` 的归档工作流。默认先 dry-run，只有明确使用 `--apply` 才能执行移动；移动完成后只能用 `--include-archive` 检查结构和归档元数据，不得借验证之名改写归档正文或出站链接。
