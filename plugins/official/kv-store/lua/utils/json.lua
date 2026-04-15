local M = {}

function M.parse(raw)
    local ok, decoded = pcall(sushi.json.decode, raw)
    if ok then
        return decoded
    end
    return nil
end

function M.encode(value)
    return sushi.json.encode(value)
end

return M
