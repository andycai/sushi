local M = {}

local function url_decode(value)
    if not value then
        return ""
    end
    local decoded = value:gsub("+", " ")
    decoded = decoded:gsub("%%(%x%x)", function(hex)
        return string.char(tonumber(hex, 16))
    end)
    return decoded
end

function M.split_path_and_query(dispatch_path)
    local source = dispatch_path or ""
    local qmark = source:find("?", 1, true)
    if not qmark then
        return source, ""
    end
    return source:sub(1, qmark - 1), source:sub(qmark + 1)
end

function M.parse_query(query)
    local out = {}
    local source = query or ""
    for key, value in string.gmatch(source, "([^&=]+)=?([^&]*)") do
        out[url_decode(key)] = url_decode(value)
    end
    return out
end

function M.read_query_value(args, key)
    local dispatch = (args and args.dispatch_path) or (args and args[1]) or ""
    local _, query = M.split_path_and_query(dispatch)
    local parsed = M.parse_query(query)
    return parsed[key]
end

function M.extract_root_id(path, prefix)
    local source = path or ""
    local stem = prefix or ""
    if source:sub(1, #stem) ~= stem then
        return ""
    end

    local tail = source:sub(#stem + 1)
    if tail == "" then
        return ""
    end

    local root = tail:match("^([^/]+)") or ""
    return url_decode(root)
end

function M.normalize_rel_path(value)
    local source = tostring(value or "")
    source = source:gsub("^/+", "")
    source = source:gsub("/+$", "")
    if source == "." then
        return ""
    end
    return source
end

function M.join_rel(parent_path, leaf)
    local parent = M.normalize_rel_path(parent_path)
    local name = M.normalize_rel_path(leaf)
    if parent == "" then
        return name
    end
    if name == "" then
        return parent
    end
    return parent .. "/" .. name
end

function M.parent_path(rel_path)
    local normalized = M.normalize_rel_path(rel_path)
    local idx = normalized:match("^.*()/")
    if not idx then
        return ""
    end
    return normalized:sub(1, idx - 1)
end

function M.file_name(rel_path)
    local normalized = M.normalize_rel_path(rel_path)
    local idx = normalized:match("^.*()/")
    if not idx then
        return normalized
    end
    return normalized:sub(idx + 1)
end

function M.clean_name(name)
    local value = tostring(name or "")
    value = value:gsub("^%s+", "")
    value = value:gsub("%s+$", "")
    value = value:gsub("/", "")
    value = value:gsub("\\", "")
    return value
end

return M
