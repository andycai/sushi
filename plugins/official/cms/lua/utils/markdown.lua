local M = {}

function M.to_html(markdown)
    return tostring(markdown or "")
end

return M
