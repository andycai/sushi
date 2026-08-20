# 1. Local Environment Guardrails (环境禁忌与红线)

*绝对禁止的行为——会破坏环境或越权*

- **禁止复合删除：** 执行器会拒绝包含 `rm -f`/`rm -rf` 风格清理的复合 shell 命令，即使目标是临时目录 -> 验证命令不附带删除步骤，临时资源使用系统目录并单独处理。
- **禁止署名：** 提交时的 commit message 包含 `Co-Authored-By`、`Signed-off-by`、`Reviewed-by` 等自动署名行 -> 不要带自动署名行

<!-- 容量上限：15-20 条。超出时合并或归档旧条目 -->
