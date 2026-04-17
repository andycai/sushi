local M = {}

local function escape_html(input)
    local out = tostring(input or "")
    out = out:gsub("&", "&amp;")
    out = out:gsub("<", "&lt;")
    out = out:gsub(">", "&gt;")
    out = out:gsub("\"", "&quot;")
    out = out:gsub("'", "&#39;")
    return out
end

function M.to_html(markdown)
    local escaped = escape_html(markdown)
    escaped = escaped:gsub("\r\n", "\n")
    escaped = escaped:gsub("\n\n+", "</p><p>")
    return "<p>" .. escaped .. "</p>"
end

return M
