local M = {}

function M.required(value)
    return value ~= nil and tostring(value) ~= ""
end

return M
