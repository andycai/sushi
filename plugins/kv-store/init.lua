-- KV Store Plugin
-- Provides API routes, admin page, and CLI commands for key-value management
-- using sushi.db.* bindings backed by SQLite.

local kv = {
    utils = {},
    infra = { db = {} },
    domain = { store = {} },
    interfaces = { api = {}, admin = {}, cli = {} },
    bootstrap = {},
}

local json_encode = sushi.json.encode

-- temporary compatibility: existing flat functions can still exist below,
-- but all new/ported functions must attach to kv.* tables.

-- JSON helpers (using native sushi bindings)
kv.utils.json_parse = function(raw)
    local ok, res = pcall(sushi.json.decode, raw)
    if ok then
        return res
    end
    return nil
end

kv.utils.html_escape = function(value)
    local text = tostring(value or "")
    text = text:gsub("&", "&amp;")
    text = text:gsub("<", "&lt;")
    text = text:gsub(">", "&gt;")
    text = text:gsub('"', "&quot;")
    text = text:gsub("'", "&#39;")
    return text
end

local function url_decode(value)
    if not value then
        return ""
    end
    local decoded = value:gsub("+", " ")
    decoded = decoded:gsub("%%(%x%x)", function(hex)
        return string.char(tonumber(hex, 16))
    end)
    return decoded
end

kv.utils.parse_form_urlencoded = function(body)
    local out = {}
    local source = body or ""
    for key, value in string.gmatch(source, "([^&=]+)=?([^&]*)") do
        out[url_decode(key)] = url_decode(value)
    end
    return out
end

kv.infra.db.query = function(sql, params)
    local ok, rows_or_err = pcall(function()
        return sushi.db.query(sql, params)
    end)
    if not ok then
        return nil, "storage_error", tostring(rows_or_err)
    end
    return rows_or_err, nil, nil
end

kv.infra.db.execute = function(sql, params)
    local ok, res_or_err = pcall(function()
        return sushi.db.execute(sql, params)
    end)
    if not ok then
        return nil, "storage_error", tostring(res_or_err)
    end
    return true, nil, nil
end

local function domain_error(kind, message)
    return nil, kind, message
end

local KV_UPSERT_SQL =
    "INSERT INTO kv_store (key, value, updated_at) VALUES (?1, ?2, datetime('now')) ON CONFLICT(key) DO UPDATE SET value = ?2, updated_at = datetime('now')"

kv.domain.store.list = function()
    local rows, kind, msg = kv.infra.db.query(
        "SELECT key, value FROM kv_store ORDER BY key",
        nil
    )
    if not rows then
        return domain_error(kind or "storage_error", msg)
    end
    return rows
end

kv.domain.store.get = function(key)
    if not key or key == "" then
        return domain_error("invalid_key", "key cannot be empty")
    end
    local rows, kind, msg = kv.infra.db.query(
        "SELECT value FROM kv_store WHERE key = ?1",
        { key }
    )
    if not rows then
        return domain_error(kind or "storage_error", msg)
    end
    if #rows == 0 then
        return domain_error("not_found", "key not found")
    end
    return { key = key, value = rows[1].value }
end

kv.domain.store.upsert = function(key, value)
    if not key or key == "" then
        return domain_error("invalid_key", "key cannot be empty")
    end
    if value == nil or value == false then
        return domain_error("invalid_value", "value cannot be empty")
    end
    local ok, kind, msg = kv.infra.db.execute(
        KV_UPSERT_SQL,
        { key, value }
    )
    if not ok then
        return domain_error(kind or "storage_error", msg)
    end
    return { key = key, value = value }
end

kv.domain.store.delete = function(key)
    if not key or key == "" then
        return domain_error("invalid_key", "key cannot be empty")
    end
    local ok, kind, msg = kv.infra.db.execute(
        "DELETE FROM kv_store WHERE key = ?1",
        { key }
    )
    if not ok then
        return domain_error(kind or "storage_error", msg)
    end
    return true
end

local function json_parse(s)
    return kv.utils.json_parse(s)
end

local function html_escape(value)
    return kv.utils.html_escape(value)
end

local function parse_form_urlencoded(body)
    return kv.utils.parse_form_urlencoded(body)
end

local function db_query(sql, params)
    local rows, err_kind, err_message = kv.infra.db.query(sql, params)
    if not rows then
        return nil, err_message
    end
    return rows
end

local function db_execute(sql, params)
    local ok, err_kind, err_message = kv.infra.db.execute(sql, params)
    if not ok then
        return nil, err_message
    end
    return ok
end

local function error_response(status, msg)
    return sushi.web.json(status, { error = tostring(msg) })
end

local function kv_upsert(key, value)
    return db_execute(
        KV_UPSERT_SQL,
        { key, value }
    )
end

-- ========================
-- API Handlers
-- ========================

-- GET /api/kv — list all entries
local function api_list()
    local rows, err = db_query("SELECT key, value FROM kv_store ORDER BY key", nil)
    if not rows then return error_response(500, err) end
    return json_encode(rows)
end

-- POST /api/kv — create entry {key, value}
local function api_create(path, body)
    local data = json_parse(body)
    if not data or not data.key or not data.value then
        return error_response(400, "missing key or value")
    end
    if data.key == "" then
        return error_response(400, "key cannot be empty")
    end
    local ok, err = kv_upsert(data.key, data.value)
    if not ok then return error_response(500, err) end
    return json_encode({ key = data.key, value = data.value })
end

-- GET /api/kv/{key} — get single entry
local function api_get_key(path)
    local key = path:match("^/api/kv/(.+)$")
    if not key then return error_response(400, "invalid path") end
    local rows, err = db_query("SELECT value FROM kv_store WHERE key = ?1", { key })
    if not rows then return error_response(500, err) end
    if #rows == 0 then return error_response(404, "key not found") end
    return json_encode({ key = key, value = rows[1].value })
end

-- PUT /api/kv/{key} — update entry
local function api_update_key(path, body)
    local key = path:match("^/api/kv/(.+)$")
    if not key then return error_response(400, "invalid path") end
    local data = json_parse(body)
    if not data or not data.value then
        return error_response(400, "missing value")
    end
    if key == "" then
        return error_response(400, "key cannot be empty")
    end
    local ok, err = kv_upsert(key, data.value)
    if not ok then return error_response(500, err) end
    return json_encode({ key = key, value = data.value })
end

-- DELETE /api/kv/{key} — delete entry
local function api_delete_key(path)
    local key = path:match("^/api/kv/(.+)$")
    if not key then return error_response(400, "invalid path") end
    local ok, err = db_execute("DELETE FROM kv_store WHERE key = ?1", { key })
    if not ok then return error_response(500, err) end
    return json_encode({ ok = true })
end

-- Main API dispatch handler for /api/kv/* routes
-- Receives a Lua table: { [1] = path, [2] = body? }
local function kv_api_dispatch(args)
    local path = args[1] or ""
    local body = args[2]

    if path == "/api/kv" then
        if body then
            return api_create(path, body)
        else
            return api_list()
        end
    elseif path:match("^/api/kv/[^/]+$") then
        if body then
            return api_update_key(path, body)
        else
            return api_get_key(path)
        end
    end
    return error_response(404, "not found")
end

-- DELETE dispatch for /api/kv/* (no body)
local function kv_api_delete_dispatch(args)
    local path = args[1] or ""
    return api_delete_key(path)
end

-- ========================
-- Admin Handler
-- ========================

local function kv_rows_partial(error_message)
    local rows, err = db_query("SELECT key, value FROM kv_store ORDER BY key", nil)
    if not rows then
        return sushi.web.render("plugins/kv-store/partials/rows.html", {
            items = {},
            error_message = error_message or tostring(err),
        })
    end

    return sushi.web.render("plugins/kv-store/partials/rows.html", {
        items = rows,
        error_message = error_message,
    })
end

local function kv_flash(level, message)
    local normalized_level = tostring(level or "success")
    local tone = "success"
    if normalized_level == "error" then
        tone = "danger"
    end
    local escaped_level = html_escape(normalized_level)
    local escaped_message = html_escape(message)
    return
        "<div class=\"ui-flash " .. tone .. "\" data-ui-flash data-level=\"" .. escaped_level .. "\" data-message=\"" .. escaped_message .. "\">"
        .. escaped_message
        .. "</div>"
end

local function kv_admin_table_partial()
    return kv_rows_partial(nil)
end

local function kv_admin_upsert_partial(args)
    local body = args[2] or ""
    local form = parse_form_urlencoded(body)
    local key = form.key or ""
    local value = form.value or ""
    local original_key = form.original_key or ""

    if key == "" then
        return kv_flash("error", "Key cannot be empty")
    end
    if value == "" then
        return kv_flash("error", "Value cannot be empty")
    end

    -- Keep key immutable in edit mode to align with UI.
    if original_key ~= "" and original_key ~= key then
        return kv_flash("error", "Changing key is not supported while editing")
    end

    local ok, err = kv_upsert(key, value)
    if not ok then
        return kv_flash("error", "Failed to save entry: " .. tostring(err))
    end
    return kv_flash("success", "Saved key: " .. key)
end

local function kv_admin_delete_partial(args)
    local body = args[2] or ""
    local form = parse_form_urlencoded(body)
    local key = form.key or ""
    if key == "" then
        return kv_flash("error", "Missing key")
    end

    local ok, err = db_execute("DELETE FROM kv_store WHERE key = ?1", { key })
    if not ok then
        return kv_flash("error", "Failed to delete key: " .. tostring(err))
    end
    return kv_flash("success", "Deleted key: " .. key)
end

-- ========================
-- CLI Handlers
-- ========================

local function cli_kv_list(args)
    local rows, err = db_query("SELECT key, value FROM kv_store ORDER BY key", nil)
    if not rows then return "Error: " .. tostring(err) end
    if #rows == 0 then return "No KV entries found." end
    local lines = {}
    for i = 1, #rows do
        lines[#lines + 1] = rows[i].key .. " = " .. rows[i].value
    end
    return table.concat(lines, "\n")
end

local function cli_kv_get(args)
    if not args[1] then return "Usage: sushi run kv-get <key>" end
    local key = args[1]
    local rows, err = db_query("SELECT value FROM kv_store WHERE key = ?1", { key })
    if not rows then return "Error: " .. tostring(err) end
    if #rows == 0 then return "Key not found: " .. key end
    return rows[1].value
end

local function cli_kv_set(args)
    if not args[1] or not args[2] then return "Usage: sushi run kv-set <key> <value>" end
    local ok, err = kv_upsert(args[1], args[2])
    if not ok then return "Error: " .. tostring(err) end
    return "OK: " .. args[1] .. " = " .. args[2]
end

local function cli_kv_delete(args)
    if not args[1] then return "Usage: sushi run kv-del <key>" end
    local ok, err = db_execute("DELETE FROM kv_store WHERE key = ?1", { args[1] })
    if not ok then return "Error: " .. tostring(err) end
    return "Deleted: " .. args[1]
end

-- ========================
-- Registration
-- ========================

function sushi.init()
    -- API routes (using wildcard prefix for /api/kv/*)
    sushi.api.route("GET", "/api/kv", kv_api_dispatch)
    sushi.api.route("GET", "/api/kv/*", kv_api_dispatch)
    sushi.api.route("POST", "/api/kv", kv_api_dispatch)
    sushi.api.route("PUT", "/api/kv/*", kv_api_dispatch)
    sushi.api.route("DELETE", "/api/kv/*", kv_api_delete_dispatch)
    sushi.api.route("GET", "/admin/partials/kv/table", kv_admin_table_partial)
    sushi.api.route("POST", "/admin/partials/kv/upsert", kv_admin_upsert_partial)
    sushi.api.route("POST", "/admin/partials/kv/delete", kv_admin_delete_partial)

    -- Admin page
    sushi.web.page("/admin/kv", "plugins/kv-store/kv.html", { title = "KV Store" })

    -- CLI commands
    sushi.cli.command("kv-list", "List all KV entries", cli_kv_list)
    sushi.cli.command("kv-get", "Get a KV entry by key", cli_kv_get)
    sushi.cli.command("kv-set", "Set a KV entry (key + value)", cli_kv_set)
    sushi.cli.command("kv-del", "Delete a KV entry by key", cli_kv_delete)

    sushi.log.info("kv-store plugin: registered API routes, admin page, and CLI commands")
end
