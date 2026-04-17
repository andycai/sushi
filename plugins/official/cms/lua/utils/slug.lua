local M = {}

function M.normalize(text)
    local value = tostring(text or ""):lower()
    value = value:gsub("[^%w%s%-_]", "")
    value = value:gsub("[%s_]+", "-")
    value = value:gsub("%-+", "-")
    value = value:gsub("^%-", ""):gsub("%-$", "")
    return value
end

return M
