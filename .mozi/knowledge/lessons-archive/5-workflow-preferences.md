# 5. Workflow & Preferences Archive (工作流与个性化偏好归档)

*已提升为稳定规则、技能合同或决策记录的工作流教训*

## 通用教训

- **文档不写技能数量：** 技能地图和项目文档写死技能或分类数量，增删后立即过时 -> 描述分类和能力，不写易漂移总数；归档：该约束已提升为 `.mozi/knowledge/rules/PROJECT.md` 的稳定项目文档规则
- **提交信息风格：** 普通提交缺少类型前缀 -> 始终使用 `feat:`、`fix:`、`docs:`、`refactor:`、`chore:` 等 Conventional Commits 前缀；归档：提交格式已由 `mozi-commit` 技能完整定义

## 当前项目教训

- **分层渐进演进：** 分离通用与项目教训时直接设计用户级目录和双层加载，放大第一步的改动与维护成本 -> 先在现有分类文件内用标题完成作用域分区，真实出现跨项目同步需求后再外置共享层；归档：该取舍已由 Agent Note `2026-08-21-split-lessons-by-scope.md` 长期记录
- **规则加载顺序：** 初始化时未在三层规则后立即读取 `lessons/` 分类文件就开始探测任务 -> 严格按 `GENERAL.md` → `PROJECT.md` → `LESSONS.md` → 全部分类文件顺序加载，完成后再并行探测任务；归档：启动加载顺序已提升到根 `AGENTS.md` 与 `.mozi/knowledge/README.md`
