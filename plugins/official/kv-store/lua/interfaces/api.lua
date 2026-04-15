local M = {}

local function api_error(kind, message)
    local status = 500
    if kind == "invalid_key" or kind == "invalid_value" then
        status = 400
    elseif kind == "not_found" then
        status = 404
    elseif kind == "storage_error" then
        status = 500
    end

    return sushi.web.json(status, { error = tostring(message or kind or "error") })
end

function M.new(deps)
    local store = deps.store
    local json = deps.json

    local api = {}

    local function api_list()
        local rows, kind, msg = store.list()
        if not rows then
            return api_error(kind, msg)
        end
        return json.encode(rows)
    end

    local function api_create(body)
        local data = json.parse(body)
        if not data or data.key == nil or data.value == nil then
            return sushi.web.json(400, { error = "missing key or value" })
        end
        if data.key == "" then
            return sushi.web.json(400, { error = "key cannot be empty" })
        end

        local entry, kind, msg = store.upsert(data.key, data.value)
        if not entry then
            return api_error(kind, msg)
        end
        return json.encode(entry)
    end

    local function api_get_key(path)
        local key = path:match("^/api/kv/(.+)$")
        if not key then
            return sushi.web.json(400, { error = "invalid path" })
        end

        local entry, kind, msg = store.get(key)
        if not entry then
            return api_error(kind, msg)
        end
        return json.encode(entry)
    end

    local function api_update_key(path, body)
        local key = path:match("^/api/kv/(.+)$")
        if not key then
            return sushi.web.json(400, { error = "invalid path" })
        end

        local data = json.parse(body)
        if not data or data.value == nil then
            return sushi.web.json(400, { error = "missing value" })
        end
        if key == "" then
            return sushi.web.json(400, { error = "key cannot be empty" })
        end

        local entry, kind, msg = store.upsert(key, data.value)
        if not entry then
            return api_error(kind, msg)
        end
        return json.encode(entry)
    end

    local function api_delete_key(path)
        local key = path:match("^/api/kv/(.+)$")
        if not key then
            return sushi.web.json(400, { error = "invalid path" })
        end

        local ok, kind, msg = store.delete(key)
        if not ok then
            return api_error(kind, msg)
        end
        return json.encode({ ok = true })
    end

    function api.dispatch(args)
        local path = args[1] or ""
        local body = args[2]

        if path == "/api/kv" then
            if body then
                return api_create(body)
            end
            return api_list()
        elseif path:match("^/api/kv/[^/]+$") then
            if body then
                return api_update_key(path, body)
            end
            return api_get_key(path)
        end

        return sushi.web.json(404, { error = "not found" })
    end

    function api.delete_dispatch(args)
        local path = args[1] or ""
        return api_delete_key(path)
    end

    return api
end

return M
