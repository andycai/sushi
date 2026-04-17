local M = {}

function M.normalize(input)
    local text = tostring(input or "")
    text = text:lower():gsub("[^%w%s%-]", ""):gsub("%s+", "-"):gsub("%-+", "-")
    text = text:gsub("^%-", ""):gsub("%-$", "")
    return text
end

return M
