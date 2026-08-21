# 3. Bug & Error Post-Mortems (技术踩坑与错误诊断)

*编译报错、运行时崩溃的具体代码坑*

## 通用教训

_暂无。_

## 当前项目教训

- **TOML 缺失表：** `toml_edit` 对缺失 section 直接使用 `document[section][field]` 会生成内联表而非标准表头 -> 写入前显式插入 `Item::Table(Table::new())`，再修改叶子字段。
- **旁路移除断言：** 测试从只改 enable intent 的 manager 旁路迁到完整 lifecycle 后仍期望注册保留并返回 `403`，实际 capability 已撤销而返回 `404` -> 删除治理旁路时同步更新测试的可见性语义，停用后的新请求应观察 owner capability 已消失。
- **字段所有权迁移：** 先替换结构体字段而未同步生产与测试中的所有旧引用，导致 crate 停留在中间态或测试无法编译 -> 迁移字段所有权前搜索完整调用链和断言点，一次替换后再执行编译与目标测试。
- **Helper 迁移调用点：** 将动态 CLI 的通用参数解析 helper 移入 builtin adapter 后，bootstrap-safe inspect 仍调用旧作用域符号，导致整个 CLI crate 无法编译 -> 移动共享 helper 前先搜索全部调用点，迁移后用显式模块路径并先做 crate 级编译。
- **单补丁重复文件操作：** 同一个 `apply_patch` 中用两个 `*** Update File` 操作修改同一路径会被验证器整体拒绝 -> 每个补丁对同一路径只声明一次操作，把多个 hunk 合并到该操作下。
- **搜索模式前导横线：** `rg` 的搜索模式以 `--dump-profile` 开头时被解析为命令选项，随后又把 `-g` 放到 `--` 后被当作路径 -> 所有 `-g` 等选项必须放在 `--` 前，使用 `rg -n -g '*.md' -- '<pattern>' <paths>` 显式分隔模式。
- **追踪输出流：** CLI 集成测试假定 tracing 写 stderr，实际 warning 混入 stdout 导致业务输出精确匹配失败 -> 日志测试同时捕获 stdout/stderr；业务输出断言应定位稳定结果行，除非命令合同明确要求纯 stdout。
- **路径所有权复用：** `PathBuf` 移入派生目标或 `spawn_blocking` 闭包后又用于校验/提示，导致所有权编译错误 -> 移动前为后续显示或校验克隆路径，或先完成所有借用操作再转移所有权。
- **测试依赖先验：** 在集成测试中直接使用同工作区其他 crate 已有的 `tempfile`，误以为当前 crate 也声明了该 dev-dependency -> 新测试先检查目标 crate 的依赖，未声明时沿用现有标准库临时目录模式，除非新增依赖确有必要。
- **存储查询参数：** 回归测试把 `Storage::query` 当作 rusqlite API 传入引用切片，又在未声明 `serde_json` 的 crate 里直接构造参数值，连续导致测试无法编译 -> 使用项目存储抽象规定的参数类型，并优先在固定测试查询中用无参数 SQL，避免为测试引入无关依赖。
- **规则加载串行：** 初始化时把三层记忆规则放进并行工具调用，破坏了规则之间的先后依赖 -> 必须先单独读取 `GENERAL.md`，再读 `PROJECT.md`，再读 `LESSONS.md`，最后才能读取分类文件和并行探测任务。
- **反引号搜索模式：** 在双引号 `rg` 模式中写 Markdown 反引号，zsh 将其当作命令替换并执行 -> 含反引号的搜索模式使用单引号，或拆成不含 shell 元字符的多个模式。
- **测试观察点稳定性：** 跨 crate 测试直接读取依赖私有字段，且新增 capability 后仍按固定下标比较排序结果 -> 使用公开 handle/snapshot 观察生命周期，并按稳定 key/path 查找目标项。
- **搜索目录先验：** `rg` 同时传入不存在的可选测试目录（本次误传仓库根 `tests/`），导致有效搜索结果伴随非零退出 -> 先用 `rg --files` 或现有目录集合确定路径，再执行跨目录搜索。
- **zsh PATH 变量：** 在 zsh 循环中使用小写 `path` 作为文件变量会覆盖特殊数组 `path`，导致后续 `rg`、`head`、`sort` 等命令突然不可用 -> shell 循环使用 `file_path` 等普通变量名，避免 `path`、`status` 等 zsh 特殊变量。
- **zsh 状态变量：** 在 zsh 复合诊断命令里用 `status=$?` 保存退出码会触发只读变量错误并中断后续输出 -> 使用 `exit_code` 等普通变量名保存退出状态。
- **TOML 标量版本字段：** 为 manifest 增加版本时误写成 `[schema_version]` table，补丁成功不代表 TOML 语义正确 -> 顶层标量必须写成 `schema_version = 1`，修改后立即用 shipped manifest 解析测试覆盖。
- **Catch-all 空尾段：** Axum 的 `/admin/{*path}` catch-all 不会替代 `/admin/` 根路由，删除静态根 handler 后动态页面返回 `404` -> 根路径需要单独注册到稳定 dispatcher，catch-all 只负责非空尾段。
- **实时锚点补丁：** 依据旧版或猜测的字段锚点修改文档/代码，实时文件结构变化后补丁未命中 -> 每次补丁前读取当前局部文本，用唯一稳定的 item/字段锚点修改，失败后停止后续验证。
- **重复块补丁定位：** 对结构相同的注册块使用上下文不足的 `apply_patch`，可能命中错误位置或把同一 namespace 方法插入两次 -> 补丁应锚定唯一测试名或独特字面量，新增 namespace 后按方法名扫描重复定义并检查相邻代码。
- **路由未命中鉴权：** 用无凭证请求断言 Axum Router 不再占用路径时，全局鉴权层会在默认 `404` 前返回 `401` -> 验证静态路由释放应携带有效凭证观察最终 `404`，并另测生产 Router 合并后的 fallback 接管。
- **rustfmt edition：** 直接对独立 Rust 文件运行 `rustfmt` 会默认按 Rust 2015 解析并在 async 代码上失败 -> 对工作区文件使用 `rustfmt --edition 2021 <files>`，避免扩大为 `cargo fmt --all`。
- **模式匹配变量遮蔽：** 在递归切片匹配中用 `left/right` 同时命名原切片和解构出的字符串，导致后续 `&left[1..]` 错切字符串 -> 解构值使用 `left_literal/right_literal` 等领域名，保留集合变量名给递归切片。
- **Cargo 测试定位：** 凭文件用途猜测不存在的 `--test` target，且对模块内单测使用短名称配 `--exact` 导致实际运行 0 项 -> 先用文件清单或 `-- --list` 确认 target/完整测试路径；不确定时去掉 `--exact` 并核对运行数量。
- **Into 参数重复消费：** 构造器里对同一个 `impl Into<String>` 参数调用两次 `.into()`，会在首次转换后移动值 -> 先转换为局部 `String`，再从局部值派生字段并最终移动入结构体。
- **unwrap_err Debug 约束：** 对成功类型未实现 `Debug` 的 `Result` 使用 `unwrap_err()` 会导致测试无法编译 -> 用显式 `match` 提取错误，避免为仅测试断言给复杂上下文补 `Debug`。
- **thiserror source 字段：** 错误枚举把普通 `String` 字段命名为 `source` 时，`thiserror` 自动要求它实现 `Error` 导致编译失败 -> 非错误链字段使用 `source_ref` 等名称，仅真实错误源使用 `source`。
- **Option 事务分支：** RuntimeHost 用 `Option<Result<_>>.transpose()` 解析 fallback owner 时丢失错误类型上下文导致编译推断失败 -> 生命周期 fallback 优先写显式 `match` 分支，避免为简单控制流引入泛型推断。
- **异步 fallback 阻塞：** 为 async manager 查询写同步 fallback 并临时引用未声明的 `futures_lite::block_on` -> 保持调用链 async，先释放短期读锁再直接 `.await` 查询，不额外引入 executor。
- **Cargo 多过滤器：** 一次给 `cargo test` 传多个位置 filter 会被解析为意外参数 -> Cargo 每次只传一个位置 filter，多目标测试使用独立命令或共同前缀过滤
- **异步发布漏 await：** 将同步 `publish` 改成 async 后调用点未补 `.await`，编译仅警告但事务测试全部失效 -> 修改函数 async 属性后立即搜索全部调用点，并把 `unused_must_use` 警告视为测试阻断错误
- **rusqlite 事务句柄：** 把 `Transaction` 当作可变 `Connection` 解引用导致借用编译失败 -> SQL 执行只需 `&Connection`，事务包装句柄持有不可变连接引用并由 `Transaction` 自身负责提交
- **模块内属性位置：** 将 `#![allow(...)]` 放在 `mod` 项之后导致 inner attribute 非法 -> 模块级 inner attribute 必须位于文件最前、任何 item 之前。
- **Rust 版本语法：** 在 edition 2021 crate 中使用 2024 才可用的 let-chain 导致编译失败 -> 写控制流前核对 workspace edition，兼容代码使用嵌套 `if let`。
- **嵌套 Arc 解引用：** 对 `RwLock<Arc<T>>` 的 guard 直接传给 `Arc::clone` 并重复 `as_ref()` 导致类型错误 -> 先通过 `&*guard` clone 内层 `Arc`，复制值时只对 `Arc<T>` 解引用一次。
- **Golden 制表符锚点：** 用文字 `\t` 作为补丁上下文匹配真实制表符 fixture，导致 `apply_patch` 不命中 -> 先用 `cat -vet` 确认字节格式，再用实际制表符或稳定的无制表符锚点修改。
- **Profile 断言分层：** 将所有非 minimal profile 统一断言为包含 Admin migration，且恢复测试让审批错误被不完整 baseline migration 链提前遮蔽 -> 数据库断言按 surface/bundle 选择；required profile 前置错误应在打开数据库和汇总 migration 前校验。
- **迁移桥接白名单：** 默认把 Lua migration 文件名当作旧 `_sushi_migrations` marker，会让其他插件的同名新 migration 被错误跳过 -> 仅为明确的历史 owner/ID 对配置 bridge，其他 Lua migration 必须正常执行并写新 catalog。
- **模块路径确认：** 按惯例修改不存在的 `lua/contract/schema/mod.rs`，导致补丁部分失败并留下半接线模块 -> 先确认 inline module 的声明文件，分文件补丁并在失败后停止验证。
- **Lua 全局遮蔽：** 插件 `app.init()` 内用 `local app` 保存业务对象，随后 `app.log` 实际访问局部对象并报 nil -> 局部变量使用领域名，保留 `app` 给运行时全局 namespace。
- **单通路测试迁移：** 删除 Lua pending registry 后先修改实现、后迁移测试观察点，导致断言从旧表读取而失败 -> 收敛双通路时同步将测试改为验证唯一的 contract registry。
- **生成补丁格式：** 将外部生成内容或 shell 命令替换原样放进单引号 heredoc，`apply_patch` 会把它当作非法 hunk 文本 -> 先生成文件内容，再用 shell 管道构造完整 `*** Begin Patch`/`*** Add File` 补丁输入，不能依赖 heredoc 内插值。

<!-- 每个作用域分区以 15-20 条为整理触发线。超出时合并或归档旧条目。 -->
