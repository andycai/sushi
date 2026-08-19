local M = {}

local function register_api_route(definition)
    definition.surface = "api"
    app.capability.register(definition)
end

function M.register(deps)
    register_api_route({ method = "GET", path = "/api/kv", handler = deps.api.dispatch, policy = "api.kv.read" })
    register_api_route({ method = "GET", path = "/api/kv/*", handler = deps.api.dispatch, policy = "api.kv.read" })
    register_api_route({ method = "POST", path = "/api/kv", handler = deps.api.dispatch, policy = "api.kv.write" })
    register_api_route({ method = "PUT", path = "/api/kv/*", handler = deps.api.dispatch, policy = "api.kv.write" })
    register_api_route({
        method = "DELETE",
        path = "/api/kv/*",
        handler = deps.api.delete_dispatch,
        policy = "api.kv.delete",
    })

    register_api_route({
        method = "GET",
        path = "/admin/partials/kv/table",
        handler = deps.admin.table_partial,
        policy = "admin.kv.manage",
    })
    register_api_route({
        method = "POST",
        path = "/admin/partials/kv/upsert",
        handler = deps.admin.upsert_partial,
        policy = "admin.kv.manage",
    })
    register_api_route({
        method = "POST",
        path = "/admin/partials/kv/delete",
        handler = deps.admin.delete_partial,
        policy = "admin.kv.manage",
    })

    app.web.page("/admin/kv", "plugins/official/kv-store/kv.html", {
        title = "KV Store",
        policy = "admin.kv.manage",
        assets = { bundles = { "workspace" } },
    })
    app.capability.register({
        surface = "menu",
        id = "kv-store.default",
        label = "KV Store",
        icon = "database",
        position = 51,
        parent_id = "host-admin.plugins",
        route = "/admin/kv",
        policy = "admin.kv.manage",
    })

    app.cli.command("kv-list", "List all KV entries", deps.cli.kv_list, { policy = "cli.kv.list" })
    app.cli.command("kv-get", "Get a KV entry by key", deps.cli.kv_get, { policy = "cli.kv.get" })
    app.cli.command("kv-set", "Set a KV entry (key + value)", deps.cli.kv_set, { policy = "cli.kv.set" })
    app.cli.command("kv-del", "Delete a KV entry by key", deps.cli.kv_del, { policy = "cli.kv.delete" })
end

return M
