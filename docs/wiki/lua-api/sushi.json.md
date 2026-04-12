# sushi.json

JSON 编解码接口。

## 可用性

- **始终可用** - 无需任何权限配置

## 方法

### `sushi.json.encode(value)`

将 Lua 值编码为 JSON 字符串。

**参数：**
- `value` (any): Lua 值（table, string, number, boolean, nil）

**返回值：**
- (string): JSON 格式字符串

**示例：**
```lua
local data = { name = "test", count = 42, active = true }
local json_str = sushi.json.encode(data)
-- -> "{\"name\":\"test\",\"count\":42,\"active\":true}"
```

---

### `sushi.json.decode(json_str)`

将 JSON 字符串解码为 Lua 值。

**参数：**
- `json_str` (string): JSON 格式字符串

**返回值：**
- (any): 解码后的 Lua 值（table, string, number, boolean, nil）

**示例：**
```lua
local json_str = '{"name":"test","count":42}'
local data = sushi.json.decode(json_str)
print(data.name)  -- -> "test"
print(data.count) -- -> 42
```

---

## 注意事项

- 只能处理 JSON 兼容的类型
- Lua table 只支持数组（整数键 1-n）和对象（字符串键）两种形式
- `nil` 值在 encode 时会被跳过
