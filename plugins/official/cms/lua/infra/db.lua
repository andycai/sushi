local M = {}

function M.query(sql, params)
    local ok, rows_or_err = pcall(function()
        return sushi.db.query(sql, params or {})
    end)
    if not ok then
        return nil, "storage_error", tostring(rows_or_err)
    end
    return rows_or_err, nil, nil
end

function M.execute(sql, params)
    local ok, result_or_err = pcall(function()
        return sushi.db.execute(sql, params or {})
    end)
    if not ok then
        return nil, "storage_error", tostring(result_or_err)
    end
    return true, nil, nil
end

return M
