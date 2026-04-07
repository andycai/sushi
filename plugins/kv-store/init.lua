-- KV Store Plugin
-- Provides API routes, admin page, and CLI commands for key-value management
-- using sushi.kv.* bindings backed by SQLite.

local JSON_CONTENT = "application/json"

-- JSON helpers (simple encode/decode for basic types)
local function json_encode(t)
    if type(t) == "string" then return '"' .. t:gsub('"', '\\"') .. '"' end
    if type(t) == "number" then return tostring(t) end
    if type(t) == "boolean" then return t and "true" or "false" end
    if t == nil then return "null" end
    if type(t) == "table" then
        -- Array-like table
        local is_array = true
        local max_idx = 0
        for k, _ in pairs(t) do
            if type(k) ~= "number" or k < 1 or math.floor(k) ~= k then
                is_array = false
                break
            end
            if k > max_idx then max_idx = k end
        end
        if is_array and max_idx == #t then
            local parts = {}
            for i = 1, #t do
                parts[i] = json_encode(t[i])
            end
            return "[" .. table.concat(parts, ",") .. "]"
        end
        -- Object-like table
        local parts = {}
        for k, v in pairs(t) do
            parts[#parts + 1] = '"' .. tostring(k) .. '":' .. json_encode(v)
        end
        return "{" .. table.concat(parts, ",") .. "}"
    end
    return "null"
end

local function json_parse(s)
    -- Very simple JSON parser for {key:"...", value:"..."} patterns
    -- Handles: strings, objects with string values
    local function parse_value(str, pos)
        pos = pos or 1
        -- skip whitespace
        while pos <= #str and str:sub(pos, pos):match("%s") do pos = pos + 1 end

        if str:sub(pos, pos) == '"' then
            -- string
            local end_pos = pos + 1
            while end_pos <= #str do
                if str:sub(end_pos, end_pos) == '"' and str:sub(end_pos - 1, end_pos - 1) ~= "\\" then
                    break
                end
                end_pos = end_pos + 1
            end
            return str:sub(pos + 1, end_pos - 1), end_pos + 1
        elseif str:sub(pos, pos) == "{" then
            -- object
            local obj = {}
            pos = pos + 1 -- skip {
            while pos <= #str do
                while pos <= #str and str:sub(pos, pos):match("%s") do pos = pos + 1 end
                if str:sub(pos, pos) == "}" then break end
                if str:sub(pos, pos) == "," then pos = pos + 1 end
                while pos <= #str and str:sub(pos, pos):match("%s") do pos = pos + 1 end
                local key, new_pos = parse_value(str, pos)
                pos = new_pos
                while pos <= #str and str:sub(pos, pos):match("%s") do pos = pos + 1 end
                if str:sub(pos, pos) == ":" then pos = pos + 1 end
                while pos <= #str and str:sub(pos, pos):match("%s") do pos = pos + 1 end
                local val, new_pos2 = parse_value(str, pos)
                pos = new_pos2
                obj[key] = val
            end
            return obj, pos + 1
        elseif str:sub(pos, pos + 3) == "null" then
            return nil, pos + 4
        elseif str:sub(pos, pos + 3) == "true" then
            return true, pos + 4
        elseif str:sub(pos, pos + 4) == "false" then
            return false, pos + 5
        end
        return nil, pos
    end
    local ok, result = pcall(parse_value, s, 1)
    if ok then return result end
    return nil
end

local function make_response(status, body)
    return json_encode({ status = status, body = body })
end

local function error_response(msg)
    return json_encode({ error = msg })
end

-- ========================
-- API Handlers
-- ========================

-- GET /api/kv — list all entries
local function api_list()
    local ok, items = pcall(function() return sushi.kv.list() end)
    if not ok then return error_response(items) end
    return json_encode(items)
end

-- POST /api/kv — create entry {key, value}
local function api_create(path, body)
    local data = json_parse(body)
    if not data or not data.key or not data.value then
        return error_response("missing key or value")
    end
    local ok, err = pcall(function() sushi.kv.set(data.key, data.value) end)
    if not ok then return error_response(err) end
    return json_encode({ key = data.key, value = data.value })
end

-- GET /api/kv/{key} — get single entry
local function api_get_key(path)
    local key = path:match("^/api/kv/(.+)$")
    if not key then return error_response("invalid path") end
    local ok, value = pcall(function() return sushi.kv.get(key) end)
    if not ok then return error_response(value) end
    if value == nil then return error_response("key not found") end
    return json_encode({ key = key, value = value })
end

-- PUT /api/kv/{key} — update entry
local function api_update_key(path, body)
    local key = path:match("^/api/kv/(.+)$")
    if not key then return error_response("invalid path") end
    local data = json_parse(body)
    if not data or not data.value then
        return error_response("missing value")
    end
    local ok, err = pcall(function() sushi.kv.set(key, data.value) end)
    if not ok then return error_response(err) end
    return json_encode({ key = key, value = data.value })
end

-- DELETE /api/kv/{key} — delete entry
local function api_delete_key(path)
    local key = path:match("^/api/kv/(.+)$")
    if not key then return error_response("invalid path") end
    local ok, err = pcall(function() sushi.kv.delete(key) end)
    if not ok then return error_response(err) end
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
    return error_response("not found")
end

-- DELETE dispatch for /api/kv/* (no body)
local function kv_api_delete_dispatch(args)
    local path = args[1] or ""
    return api_delete_key(path)
end

-- ========================
-- Admin Handler
-- ========================

local KV_ADMIN_HTML = [[<!DOCTYPE html>
<html lang="en">
<head><meta charset="UTF-8"><meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>KV Store — Sushi Admin</title>
<script defer src="https://unpkg.com/alpinejs@3.14.1/dist/cdn.min.js"></script>
<script src="https://cdn.tailwindcss.com/3.4.17"></script>
</head>
<body class="bg-gray-100 min-h-screen">
<div class="flex h-screen">
  <nav class="w-60 bg-gray-900 text-white flex-shrink-0">
    <div class="p-4 text-xl font-bold border-b border-gray-700">Sushi Admin</div>
    <div class="mt-4">
      <a href="/admin/" class="block px-4 py-2 hover:bg-gray-700">Dashboard</a>
      <a href="/admin/plugins" class="block px-4 py-2 hover:bg-gray-700">Plugins</a>
      <a href="/admin/users" class="block px-4 py-2 hover:bg-gray-700">Users</a>
      <a href="/admin/config" class="block px-4 py-2 hover:bg-gray-700">Config</a>
      <a href="/admin/logs" class="block px-4 py-2 hover:bg-gray-700">Logs</a>
      <a href="/admin/kv" class="block px-4 py-2 bg-gray-700">KV Store</a>
    </div>
  </nav>
  <main class="flex-1 p-6 overflow-auto" x-data="kvPage()">
    <div class="flex justify-between items-center mb-6">
      <h1 class="text-2xl font-bold">KV Store</h1>
      <button @click="showAddModal = true; form = {key: '', value: ''}"
        class="bg-blue-600 text-white px-4 py-2 rounded hover:bg-blue-700">Add Entry</button>
    </div>
    <div x-show="error" class="mb-4 p-3 bg-red-100 text-red-700 rounded" x-text="error"></div>
    <div x-show="loading" class="text-gray-500">Loading...</div>
    <div x-show="!loading">
      <div class="bg-white rounded shadow overflow-hidden">
        <table class="w-full">
          <thead class="bg-gray-50 border-b">
            <tr>
              <th class="px-4 py-3 text-left text-sm font-medium text-gray-600">Key</th>
              <th class="px-4 py-3 text-left text-sm font-medium text-gray-600">Value</th>
              <th class="px-4 py-3 text-right text-sm font-medium text-gray-600">Actions</th>
            </tr>
          </thead>
          <tbody>
            <template x-for="item in items" :key="item.key">
              <tr class="border-b hover:bg-gray-50">
                <td class="px-4 py-3 font-mono text-sm" x-text="item.key"></td>
                <td class="px-4 py-3 font-mono text-sm truncate max-w-xs" x-text="item.value"></td>
                <td class="px-4 py-3 text-right">
                  <button @click="editItem(item)" class="text-blue-600 hover:text-blue-800 mr-3 text-sm font-medium">Edit</button>
                  <button @click="deleteItem(item.key)" class="text-red-600 hover:text-red-800 text-sm font-medium">Delete</button>
                </td>
              </tr>
            </template>
            <tr x-show="items.length === 0">
              <td colspan="3" class="px-4 py-8 text-center text-gray-500">No entries found. Add one to get started.</td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
    <div x-show="showAddModal || showEditModal" x-transition
      class="fixed inset-0 bg-black/50 flex items-center justify-center z-50" style="display: none">
      <div class="bg-white rounded-lg shadow-xl w-full max-w-md mx-4">
        <div class="px-6 py-4 border-b">
          <h2 class="text-lg font-semibold" x-text="showAddModal ? 'Add Entry' : 'Edit Entry'"></h2>
        </div>
        <form @submit.prevent="saveItem()" class="px-6 py-4 space-y-4">
          <div>
            <label class="block text-sm font-medium text-gray-700 mb-1">Key</label>
            <input type="text" x-model="form.key" :disabled="showEditModal"
              class="w-full border rounded px-3 py-2 focus:ring-2 focus:ring-blue-500 focus:outline-none"
              :class="showEditModal ? 'bg-gray-100' : ''" required>
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 mb-1">Value</label>
            <input type="text" x-model="form.value"
              class="w-full border rounded px-3 py-2 focus:ring-2 focus:ring-blue-500 focus:outline-none" required>
          </div>
          <div class="flex justify-end gap-3 pt-2">
            <button type="button" @click="closeModal()"
              class="px-4 py-2 border rounded hover:bg-gray-50">Cancel</button>
            <button type="submit"
              class="px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700">Save</button>
          </div>
        </form>
      </div>
    </div>
  </main>
</div>
<script>
function kvPage() {
  return {
    items: [],
    loading: true,
    error: null,
    showAddModal: false,
    showEditModal: false,
    form: { key: '', value: '' },
    async init() { await this.loadItems(); this.loading = false; },
    async loadItems() {
      try {
        const resp = await fetch('/api/kv');
        if (!resp.ok) throw new Error('Failed to load entries');
        this.items = await resp.json();
        this.error = null;
      } catch (e) {
        this.error = 'Could not load KV entries: ' + e.message;
      }
    },
    editItem(item) {
      this.form = { key: item.key, value: item.value };
      this.showEditModal = true;
    },
    async deleteItem(key) {
      if (!confirm('Delete "' + key + '"?')) return;
      try {
        const resp = await fetch('/api/kv/' + encodeURIComponent(key), { method: 'DELETE' });
        if (!resp.ok) throw new Error('Failed to delete');
        await this.loadItems();
      } catch (e) {
        this.error = 'Failed to delete: ' + e.message;
      }
    },
    async saveItem() {
      try {
        let resp;
        if (this.showAddModal) {
          resp = await fetch('/api/kv', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(this.form)
          });
        } else {
          resp = await fetch('/api/kv/' + encodeURIComponent(this.form.key), {
            method: 'PUT',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ value: this.form.value })
          });
        }
        if (!resp.ok) throw new Error('Failed to save');
        this.closeModal();
        await this.loadItems();
      } catch (e) {
        this.error = 'Failed to save: ' + e.message;
      }
    },
    closeModal() {
      this.showAddModal = false;
      this.showEditModal = false;
      this.form = { key: '', value: '' };
    }
  };
}
</script>
</body></html>]]

-- ========================
-- CLI Handlers
-- ========================

local function cli_kv_list(args)
    local ok, items = pcall(function() return sushi.kv.list() end)
    if not ok then return "Error: " .. items end
    if #items == 0 then return "No KV entries found." end
    local lines = {}
    for i = 1, #items do
        lines[#lines + 1] = items[i].key .. " = " .. items[i].value
    end
    return table.concat(lines, "\n")
end

local function cli_kv_get(args)
    if not args[1] then return "Usage: sushi run kv-get <key>" end
    local key = args[1]
    local ok, value = pcall(function() return sushi.kv.get(key) end)
    if not ok then return "Error: " .. value end
    if value == nil then return "Key not found: " .. key end
    return value
end

local function cli_kv_set(args)
    if not args[1] or not args[2] then return "Usage: sushi run kv-set <key> <value>" end
    local ok, err = pcall(function() sushi.kv.set(args[1], args[2]) end)
    if not ok then return "Error: " .. err end
    return "OK: " .. args[1] .. " = " .. args[2]
end

local function cli_kv_delete(args)
    if not args[1] then return "Usage: sushi run kv-del <key>" end
    local ok, err = pcall(function() sushi.kv.delete(args[1]) end)
    if not ok then return "Error: " .. err end
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

    -- Admin page
    sushi.admin.page("/admin/kv", "KV Store", function()
        return KV_ADMIN_HTML
    end)

    -- CLI commands
    sushi.cli.command("kv-list", "List all KV entries", cli_kv_list)
    sushi.cli.command("kv-get", "Get a KV entry by key", cli_kv_get)
    sushi.cli.command("kv-set", "Set a KV entry (key + value)", cli_kv_set)
    sushi.cli.command("kv-del", "Delete a KV entry by key", cli_kv_delete)

    sushi.log.info("kv-store plugin: registered API routes, admin page, and CLI commands")
end
