local M = {}

function M.new(deps)
    local store = deps.store

    local cli = {}

    function cli.kv_list(_args)
        local rows, _, msg = store.list()
        if not rows then
            return "Error: " .. tostring(msg)
        end
        if #rows == 0 then
            return "No KV entries found."
        end

        local lines = {}
        for i = 1, #rows do
            lines[#lines + 1] = rows[i].key .. " = " .. rows[i].value
        end
        return table.concat(lines, "\n")
    end

    function cli.kv_get(args)
        if not args[1] then
            return "Usage: sushi kv-get <key>"
        end

        local key = args[1]
        local entry, kind, msg = store.get(key)
        if not entry then
            if kind == "not_found" then
                return "Key not found: " .. key
            end
            return "Error: " .. tostring(msg)
        end
        return entry.value
    end

    function cli.kv_set(args)
        if not args[1] or not args[2] then
            return "Usage: sushi kv-set <key> <value>"
        end

        local entry, _, msg = store.upsert(args[1], args[2])
        if not entry then
            return "Error: " .. tostring(msg)
        end
        return "OK: " .. args[1] .. " = " .. args[2]
    end

    function cli.kv_del(args)
        if not args[1] then
            return "Usage: sushi kv-del <key>"
        end

        local ok, _, msg = store.delete(args[1])
        if not ok then
            return "Error: " .. tostring(msg)
        end
        return "Deleted: " .. args[1]
    end

    return cli
end

return M
