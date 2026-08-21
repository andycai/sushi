# 代理初始化

执行任何操作前，按以下顺序加载项目知识：

1. [`.mozi/knowledge/README.md`](.mozi/knowledge/README.md)
2. [`.mozi/knowledge/rules/GENERAL.md`](.mozi/knowledge/rules/GENERAL.md)
3. [`.mozi/knowledge/rules/PROJECT.md`](.mozi/knowledge/rules/PROJECT.md)
4. [`.mozi/knowledge/LESSONS.md`](.mozi/knowledge/LESSONS.md) 及其索引的全部活跃分类文件

归档教训和其他知识按任务需要读取，不进入默认启动上下文。

## 自我纠错

遇到用户纠正、命令失败、实现缺陷、中途发现错误或可复用的更优做法时，立即使用 `mozi-reflect` 完成过滤、重复诊断、知识路由和当前任务修正。会话结束前再次检查是否遗漏了实际发生且未来可能复现的纠错事件。

## Agent Notes

除纯机械编辑和不涉及决策内容的局部编辑外，所有非平凡变更都必须在同一变更中新增或更新至少一份 Agent Note；这是代理行为约定，不接入自动门禁。

Agent Note 的唯一生命周期入口是 `mozi-agent-notes`，[`.mozi/knowledge/decisions/README.md`](.mozi/knowledge/decisions/README.md) 是其格式与生命周期的唯一权威规范。
