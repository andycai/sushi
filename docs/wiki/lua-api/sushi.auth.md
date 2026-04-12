# sushi.auth

认证接口。

## 可用性

- **始终可用** - 无需任何权限配置

## 方法

### `sushi.auth.verify_token(token)`

验证 JWT token 并返回 claims。

**参数：**
- `token` (string): JWT token 字符串（通常为 `Bearer <token>` 格式）

**返回值：**
- (table|nil): 成功返回包含以下字段的 table，失败返回 nil
  - `sub` (string): 用户 ID
  - `username` (string): 用户名
  - `role` (string): 角色
  - `token_type` (string): token 类型

**示例：**
```lua
local token = "eyJhbGciOiJIUzI1NiIs..."
local claims = sushi.auth.verify_token(token)

if claims then
    print("User: " .. claims.username)
    print("Role: " .. claims.role)
else
    print("Invalid token")
end
```

---

### `sushi.auth.hash_password(password)` (规划中)

密码哈希功能。

> 注意：此方法尚未实现。

---

## 错误处理

- 无效 token → 返回 `nil`
- 过期 token → 返回 `nil`
- 格式错误 → 返回 `nil`
