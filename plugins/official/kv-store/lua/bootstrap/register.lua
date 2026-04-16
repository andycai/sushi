local M = {}

function M.register(deps)
    sushi.api.route("GET", "/api/kv", deps.api.dispatch, { policy = "api.kv.read" })
    sushi.api.route("GET", "/api/kv/*", deps.api.dispatch, { policy = "api.kv.read" })
    sushi.api.route("POST", "/api/kv", deps.api.dispatch, { policy = "api.kv.write" })
    sushi.api.route("PUT", "/api/kv/*", deps.api.dispatch, { policy = "api.kv.write" })
    sushi.api.route("DELETE", "/api/kv/*", deps.api.delete_dispatch, { policy = "api.kv.delete" })

    sushi.api.route("GET", "/admin/partials/kv/table", deps.admin.table_partial, { policy = "api.kv.read" })
    sushi.api.route("POST", "/admin/partials/kv/upsert", deps.admin.upsert_partial, { policy = "api.kv.write" })
    sushi.api.route("POST", "/admin/partials/kv/delete", deps.admin.delete_partial, { policy = "api.kv.delete" })

    sushi.web.page("/admin/kv", "plugins/official/kv-store/kv.html", {
        title = "KV Store",
        policy = "admin.kv.read",
        assets = { bundles = { "workspace" } },
    })

    sushi.cli.command("kv-list", "List all KV entries", deps.cli.kv_list, { policy = "cli.kv.list" })
    sushi.cli.command("kv-get", "Get a KV entry by key", deps.cli.kv_get, { policy = "cli.kv.get" })
    sushi.cli.command("kv-set", "Set a KV entry (key + value)", deps.cli.kv_set, { policy = "cli.kv.set" })
    sushi.cli.command("kv-del", "Delete a KV entry by key", deps.cli.kv_del, { policy = "cli.kv.delete" })
end

return M
