local M = {}

function M.register(deps)
    sushi.api.route("GET", "/api/kv", deps.api.dispatch)
    sushi.api.route("GET", "/api/kv/*", deps.api.dispatch)
    sushi.api.route("POST", "/api/kv", deps.api.dispatch)
    sushi.api.route("PUT", "/api/kv/*", deps.api.dispatch)
    sushi.api.route("DELETE", "/api/kv/*", deps.api.delete_dispatch)

    sushi.api.route("GET", "/admin/partials/kv/table", deps.admin.table_partial)
    sushi.api.route("POST", "/admin/partials/kv/upsert", deps.admin.upsert_partial)
    sushi.api.route("POST", "/admin/partials/kv/delete", deps.admin.delete_partial)

    sushi.web.page("/admin/kv", "plugins/official/kv-store/kv.html", {
        title = "KV Store",
        assets = { bundles = { "workspace" } },
    })

    sushi.cli.command("kv-list", "List all KV entries", deps.cli.kv_list)
    sushi.cli.command("kv-get", "Get a KV entry by key", deps.cli.kv_get)
    sushi.cli.command("kv-set", "Set a KV entry (key + value)", deps.cli.kv_set)
    sushi.cli.command("kv-del", "Delete a KV entry by key", deps.cli.kv_del)
end

return M
