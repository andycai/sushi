local M = {}

function M.query(sql, params)
    return sushi.db.query(sql, params or {})
end

function M.exec(sql, params)
    return sushi.db.exec(sql, params or {})
end

return M
