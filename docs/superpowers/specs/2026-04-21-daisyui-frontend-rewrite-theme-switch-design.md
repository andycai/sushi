# Sushi 前端 DaisyUI 重写与主题切换设计方案

> 日期: 2026-04-21
> 状态: Approved
> 范围: 全站后台模板（含官方插件）

## 1. 背景与目标

当前后台 UI 主要依赖自定义样式（`web/static/admin/css/admin.css`）与 Tailwind 生成样式混用，视觉与组件语义不统一。项目已具备 Tailwind v4 基础，但 daisyUI 接入方式为本地 JS bundle 路径引用，不是标准 Node 包构建链路。

本次目标：

1. 以 daisyUI + Tailwind v4 重建后台前端样式体系；
2. 支持 `light/dark` 两主题切换并持久化；
3. 保持 Rust/Lua 后端接口与页面行为契约不变；
4. 统一 CSS 输出到 `web/static/css/style.css`；
5. 彻底移除对 `web/static/admin/css/admin.css` 的运行时依赖。

## 2. 范围与非目标

### In Scope

- `web/templates/**` 管理后台模板与片段
- `plugins/official/**/web/templates/**` 官方插件模板与片段（KV/CMS/File Browser）
- `scripts/watch-css.sh` 与 `scripts/compile-css.sh`
- `web/static/css/input.css`（Tailwind + daisyUI 插件声明与模板扫描配置）
- `base.html` 主题切换入口与初始化逻辑

### Out of Scope

- 不改 Rust/Lua API 路由、权限与业务处理逻辑
- 不新增 `light/dark` 之外主题
- 不实现系统主题自动跟随（`prefers-color-scheme`）
- 不重构插件后端 Lua/Rust 数据结构

## 3. 方案概览（推荐方案：重建式迁移）

采用“契约不变、结构重建”的方式执行：

1. 先完成构建链路 Node 化与主题基础能力；
2. 再进行全模板深度重构，将旧 `ui-*` 视觉体系替换为 daisyUI 组件语义；
3. 保留 HTMX、Alpine、`data-*`、`id`、片段路径和路由契约，确保行为等价。

该方案改动面大，但最终一致性最好，后续维护成本最低。

## 4. 架构与组件设计

### 4.1 CSS 构建链路（Node 化）

- 新增 `package.json` 并引入：
  - `tailwindcss`
  - `@tailwindcss/cli`
  - `daisyui`
- 统一通过 `pnpm exec tailwindcss` 构建 CSS：
  - `./scripts/compile-css.sh`：一次性构建
  - `./scripts/watch-css.sh`：`--watch` 实时构建
- 输出保持不变：`web/static/css/style.css`

### 4.2 Tailwind v4 输入文件规范

`web/static/css/input.css` 采用 v4 CSS-first 配置：

- `@import "tailwindcss";`
- `@plugin "daisyui";`
- `@source` 显式扫描模板目录（相对 `web/static/css/input.css`）：
  - `../../templates/**/*.html`
  - `../../../plugins/**/web/templates/**/*.html`

保证模板类名变化可被 watch 模式及时捕获并导出。

### 4.3 主题系统（light/dark）

- 主题状态挂载在 `<html data-theme="light|dark">`
- 初始主题决策：`localStorage` 值优先，否则默认 `light`
- 顶部提供全局 Theme Toggle（在 `base.html`）
- 切换动作：
  1. 更新 `data-theme`
  2. 写入 `localStorage`
  3. 同步按钮可访问性状态
- 为避免首屏闪烁，在 CSS/脚本加载前插入最小初始化脚本

### 4.4 模板组件映射策略

- 以 daisyUI 组件为主：
  - `btn`、`card`、`table`、`input/select/textarea`、`badge`、`alert`、`modal`、`drawer`
- 允许少量 `@layer components` 作为桥接样式，但不得形成第二套视觉系统
- 移除模板中对 `admin.css` 的依赖

### 4.5 页面与插件统一策略

- `web/templates/base.html` 作为统一壳层
- Admin 页面（dashboard/users/roles/permissions/menus/logs/config/plugins/login）全部迁移
- 官方插件页面（KV/CMS/File Browser）统一主题与组件语义：
  - 若页面已继承 `base.html`，直接受益
  - 独立模板（如 File Browser）需纳入统一样式与主题机制

## 5. 关键数据流与运行流

### 5.1 构建数据流

1. 修改模板 HTML 或 `input.css`
2. `watch-css.sh` 触发 Tailwind v4 重新扫描 `@source`
3. daisyUI 插件产物与 Tailwind utilities 一并输出到 `web/static/css/style.css`
4. 页面直接使用最新样式

### 5.2 主题运行流

1. 页面加载时先执行内联初始化脚本
2. 读取 `localStorage.theme` 并设置 `<html data-theme>`
3. 页面渲染使用 daisyUI 主题 token
4. 用户点击 toggle 后更新主题与持久化
5. HTMX 局部刷新不影响全局主题状态

## 6. 错误处理与回退策略

- `localStorage` 不可用：回退 `light`，切换仅当前会话生效
- 非法主题值：忽略并回退 `light`
- `pnpm` 未安装：脚本输出明确错误提示并中止
- watch 场景未触发（极少数环境文件监听异常）：保留 `compile-css.sh` 作为手动回退
- 插件页面无法继承壳层时：最小化补充适配层，保证视觉与主题一致

## 7. 验收标准

必须全部满足：

1. `./scripts/compile-css.sh` 成功生成 `web/static/css/style.css`，且包含 daisyUI 类（如 `.btn`）
2. `./scripts/watch-css.sh` 运行后，修改模板 HTML 可触发样式更新
3. 全站后台页面（含官方插件）支持 `light/dark` 切换并刷新后保持
4. 模板不再引用 `web/static/admin/css/admin.css`
5. 现有关键行为不回归（登录、菜单导航、HTMX 局部刷新、CMS 工作台、File Browser 主流程）

## 8. 测试与验证计划

构建与样式验证：

- `pnpm install`
- `./scripts/compile-css.sh`
- `./scripts/watch-css.sh`

后端模板契约与集成验证：

- `cargo test -p sushi-core --test template_service -q`
- `cargo test -p sushi-admin --test admin_web -q`
- `cargo test --workspace -q`

静态检查：

- `rg "admin/css/admin.css" web/templates plugins/official/*/web/templates` 应无命中

## 9. 交付分期

为降低风险，按两阶段交付：

### Phase 1（基础设施）

- Node 构建链路接入（pnpm + tailwind + daisyui）
- `compile/watch` 脚本切换
- 主题初始化与全局切换入口

### Phase 2（全量迁移）

- Admin 与官方插件模板全量重构
- 旧样式依赖清理
- 回归测试与视觉一致性校验

## 10. 风险与缓解

- **风险：** 模板改动范围大导致回归点增多  
  **缓解：** 保留 HTML 契约钩子不变，按页面清单逐项回归

- **风险：** File Browser 独立模板适配复杂  
  **缓解：** 先统一主题容器，再逐步替换局部结构

- **风险：** 迁移期类名遗漏导致样式缺失  
  **缓解：** 通过 `@source` 扫描覆盖全模板目录，并进行核心页面冒烟测试

---

本设计文档已覆盖架构、组件、数据流、错误处理、测试与验收标准，可直接进入实施计划编写。
