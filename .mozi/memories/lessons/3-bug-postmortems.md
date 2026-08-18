# 3. Bug & Error Post-Mortems (技术踩坑与错误诊断)

*编译报错、运行时崩溃的具体代码坑*

- **Cargo 多过滤器：** 一次给 `cargo test` 传多个位置 filter 会被解析为意外参数 -> Cargo 每次只传一个位置 filter，多目标测试使用独立命令或共同前缀过滤
- **异步发布漏 await：** 将同步 `publish` 改成 async 后调用点未补 `.await`，编译仅警告但事务测试全部失效 -> 修改函数 async 属性后立即搜索全部调用点，并把 `unused_must_use` 警告视为测试阻断错误
- **rusqlite 事务句柄：** 把 `Transaction` 当作可变 `Connection` 解引用导致借用编译失败 -> SQL 执行只需 `&Connection`，事务包装句柄持有不可变连接引用并由 `Transaction` 自身负责提交
- **模块内属性位置：** 将 `#![allow(...)]` 放在 `mod` 项之后导致 inner attribute 非法 -> 模块级 inner attribute 必须位于文件最前、任何 item 之前。
- **Rust 版本语法：** 在 edition 2021 crate 中使用 2024 才可用的 let-chain 导致编译失败 -> 写控制流前核对 workspace edition，兼容代码使用嵌套 `if let`。
- **嵌套 Arc 解引用：** 对 `RwLock<Arc<T>>` 的 guard 直接传给 `Arc::clone` 并重复 `as_ref()` 导致类型错误 -> 先通过 `&*guard` clone 内层 `Arc`，复制值时只对 `Arc<T>` 解引用一次。

<!-- 容量上限：15-20 条。超出时合并或归档旧条目 -->
