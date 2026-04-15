local M = {}

function M.new(deps)
    local store = deps.store
    local form = deps.form
    local html = deps.html

    local admin = {}

    local function safe_key(key)
        if html and html.escape then
            return html.escape(key)
        end
        return tostring(key or "")
    end

    local function kv_rows_partial(error_message)
        local rows, _, msg = store.list()
        if not rows then
            return sushi.web.render("plugins/official/kv-store/partials/rows.html", {
                items = {},
                error_message = error_message or tostring(msg),
            })
        end

        return sushi.web.render("plugins/official/kv-store/partials/rows.html", {
            items = rows,
            error_message = error_message,
        })
    end

    local function kv_flash(level, message)
        return sushi.web.render("plugins/official/kv-store/partials/flash.html", {
            level = tostring(level or "success"),
            message = tostring(message or ""),
        })
    end

    function admin.table_partial()
        return kv_rows_partial(nil)
    end

    function admin.upsert_partial(args)
        local body = args[2] or ""
        local parsed = form.parse_urlencoded(body)
        local key = parsed.key or ""
        local value = parsed.value or ""
        local original_key = parsed.original_key or ""

        if key == "" then
            return kv_flash("error", "Key cannot be empty")
        end
        if value == "" then
            return kv_flash("error", "Value cannot be empty")
        end

        if original_key ~= "" and original_key ~= key then
            return kv_flash("error", "Changing key is not supported while editing")
        end

        local entry, _, msg = store.upsert(key, value)
        if not entry then
            return kv_flash("error", "Failed to save entry: " .. tostring(msg))
        end
        return kv_flash("success", "Saved key: " .. safe_key(key))
    end

    function admin.delete_partial(args)
        local body = args[2] or ""
        local parsed = form.parse_urlencoded(body)
        local key = parsed.key or ""
        if key == "" then
            return kv_flash("error", "Missing key")
        end

        local ok, _, msg = store.delete(key)
        if not ok then
            return kv_flash("error", "Failed to delete key: " .. tostring(msg))
        end
        return kv_flash("success", "Deleted key: " .. safe_key(key))
    end

    return admin
end

return M
