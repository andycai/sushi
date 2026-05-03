local M = {}

function M.parse(raw)
    local ok, decoded = pcall(app.json.decode, raw)
    if ok then
        return decoded
    end
    return nil
end

function M.encode(value)
    return app.json.encode(value)
end

return M
