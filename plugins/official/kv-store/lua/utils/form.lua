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

function M.parse_urlencoded(body)
    local out = {}
    local source = body or ""
    for key, value in string.gmatch(source, "([^&=]+)=?([^&]*)") do
        out[url_decode(key)] = url_decode(value)
    end
    return out
end

return M
