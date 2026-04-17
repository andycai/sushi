local M = {}
local STATUS = { draft = true, published = true }

function M.validate_status(value)
    if not STATUS[value] then
        return nil, "invalid_status", "status must be draft or published"
    end
    return value
end

function M.require_non_empty(value, field)
    local text = tostring(value or "")
    if text == "" then
        return nil, "invalid_" .. field, field .. " cannot be empty"
    end
    return text
end

return M
