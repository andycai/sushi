local M = {}

local KV_UPSERT_SQL = "INSERT INTO kv_store (key, value, updated_at) VALUES (?1, ?2, datetime('now')) ON CONFLICT(key) DO UPDATE SET value = ?2, updated_at = datetime('now')"

local function domain_error(kind, message)
    return nil, kind, message
end

function M.new(deps)
    local db = deps.db

    local store = {}

    function store.list()
        local rows, kind, msg = db.query("SELECT key, value FROM kv_store ORDER BY key", nil)
        if not rows then
            return domain_error(kind or "storage_error", msg)
        end
        return rows
    end

    function store.get(key)
        if not key or key == "" then
            return domain_error("invalid_key", "key cannot be empty")
        end

        local rows, kind, msg = db.query("SELECT value FROM kv_store WHERE key = ?1", { key })
        if not rows then
            return domain_error(kind or "storage_error", msg)
        end
        if #rows == 0 then
            return domain_error("not_found", "key not found")
        end

        return { key = key, value = rows[1].value }
    end

    function store.upsert(key, value)
        if not key or key == "" then
            return domain_error("invalid_key", "key cannot be empty")
        end
        if value == nil or value == false then
            return domain_error("invalid_value", "value cannot be empty")
        end

        local ok, kind, msg = db.execute(KV_UPSERT_SQL, { key, value })
        if not ok then
            return domain_error(kind or "storage_error", msg)
        end

        return { key = key, value = value }
    end

    function store.delete(key)
        if not key or key == "" then
            return domain_error("invalid_key", "key cannot be empty")
        end

        local ok, kind, msg = db.execute("DELETE FROM kv_store WHERE key = ?1", { key })
        if not ok then
            return domain_error(kind or "storage_error", msg)
        end

        return true
    end

    return store
end

return M
