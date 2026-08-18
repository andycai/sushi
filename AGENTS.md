# 代理初始化

在执行任何操作前，必须按顺序加载 `.mozi/memories/` 下的三层规则体系：

1. **GENERAL.md** — 通用行为准则与编码哲学
2. **PROJECT.md** — 技术栈、架构与项目约定
3. **LESSONS.md** — 经验教训、偏好与环境红线（动态知识库，含分类文件索引）

## 自我纠错（从错误中学习）

**每个错误都是成长的信号。** 遇到以下任一情况，立即执行记录：

| 触发信号 | 示例 |
|---------|------|
| 用户纠正输出 | "你错了"、"不对"、"改成..." |
| 命令执行失败 | 测试失败、编译报错、git 操作失败 |
| 中途意识到错误 | 方向错了、误解了需求 |
| 发现更优工作方式 | 换个顺序更快、换个工具更好 |

**过滤条件**：未来 3 个月内可能再次发生才记录。一次性拼写错误当场修正即可。

**执行方式**：
1. 读取 `.mozi/memories/lessons/` 下对应的分类文件
2. 按格式 `- **标题：** 发生了什么 -> 正确做法` 追加到文件顶部
3. 在当前任务中立即应用该教训
4. 回复 `已记录教训：<标题>`

详细协议见 `Skill: mozi-learn`。

### 结束前检查

**每个会话结束前**，自查是否遗漏了应记录的教训。如有遗漏，立即补记。

<claude-mem-context>
# Memory Context

# [sushi] recent context, 2026-04-22 5:00pm GMT+8

Legend: 🎯session 🔴bugfix 🟣feature 🔄refactor ✅change 🔵discovery ⚖️decision
Format: ID TIME TYPE TITLE
Fetch details: get_observations([IDs]) | Search: mem-search skill

Stats: 50 obs (8,687t read) | 855,719t work | 99% savings

### Apr 22, 2026
411 11:41a 🔴 File Browser UI Bug Reports
412 11:43a 🔵 File Browser UI Architecture
413 " 🔴 Fixed Editor Panel Flexbox Layout
414 11:44a 🔴 Fixed Directory Tree Layout on Expand
415 " 🔴 File Browser UI Bugs Fixed and Verified
416 11:45a ✅ File Browser Bug Fixes Ready
417 " ✅ Full Test Suite Passes
418 11:47a 🔴 Full Workspace Test Suite Passes
419 11:52a 🔴 File browser UI bugs identified and being fixed
420 " 🔵 Sushi web server runs on 127.0.0.1:3008
421 " 🔵 Sushi project structure discovered
422 11:53a 🔵 File browser UI is running and accessible
423 " 🔵 File browser shows runtime error on file open
424 12:01p 🔴 Directory tree rendering bug
425 12:02p 🔵 File browser JS uses extractTreeChildrenMarkup
426 12:03p 🔵 File browser roots behave differently
427 12:04p 🔴 Fixed directory tree rendering in file browser
428 12:21p ✅ Directory tree UI compactness improvements
429 " 🔵 File browser plugin current structure
430 12:22p ✅ Toolbar buttons converted to SVG icons
431 " ✅ Patch applied to file_browser.html
432 12:23p ✅ Directory tree made more compact
433 " 🟣 File/folder name character limit implemented
434 12:24p 🔵 Tests pass after compact UI changes
435 " 🔵 UI verification screenshot captured
436 " 🔵 UI verification screenshot reviewed
437 12:25p ✅ Development server restarted for testing
438 " 🔵 File browser plugin successfully loaded and UI verified
439 1:34p 🔴 Fix wallet creation screen UI issue
440 " 🔴 Fixed file browser list view layout
441 1:35p 🔴 Enhanced file browser CSS layout rules
442 " 🔴 Fixed CSS attribute selector syntax
443 " ✅ File browser plugin tests passed
444 1:41p 🔴 File browser plugin layout fixes verified
445 " 🔄 File browser compact layout refinements
446 1:42p 🔵 File browser server running on localhost:3008
448 1:49p 🔄 File browser HTML structure migrated from ul/li to div
449 " 🔄 Removed legacy group class from new div structure
450 1:50p 🔴 CSS hover-based download button visibility
451 2:08p 🔴 UI fixes for row spacing and hover state
452 " 🔴 Fixed row spacing and hover states in file browser CSS
453 2:09p 🔴 File browser CSS fixes verified with visual testing
454 2:10p 🔴 Added hover background for file browser node label
455 2:15p 🔴 Enhanced file browser hover states with smooth transitions
456 2:18p 🔴 Committed file browser UI fixes
457 2:20p 🟣 Refactored admin partial route authorization to use policy-based access control
458 2:32p ✅ Sushi API router significantly expanded with new endpoints
459 2:34p 🔴 Admin plugin routes now use admin policy surface
460 " ✅ Graphify knowledge graph auto-rebuilt after commit
461 " ✅ AGENTS.md updated alongside router fix and graph rebuild

Access 856k tokens of past work via get_observations([IDs]) or mem-search skill.
</claude-mem-context>
