# 4. Architecture & Design Patterns Archive (架构规范与设计约束归档)

*已由当前决策机制取代的架构教训*

## 通用教训

_暂无。_

## 当前项目教训

- **ADR 集中归档：** ADR 记录跨功能决策，放入 feature 产物目录会限制其共享范围 -> 统一存放在 `.mozi/memories/adr/`，文件名包含 topic，编号全局递增；归档：独立 ADR 机制已退出，长期决策统一由 `.mozi/knowledge/decisions/` 下的 Agent Note 管理
