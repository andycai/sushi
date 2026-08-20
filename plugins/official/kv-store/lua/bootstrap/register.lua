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

    app.capability.register({
        surface = "web",
        kind = "page",
        path = "/admin/kv",
        title = "KV Store",
        handler = function()
            return app.web.render("plugins/official/kv-store/kv.html")
        end,
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

    app.capability.register({
        surface = "cli",
        name = "kv-list",
        description = "List all KV entries",
        handler = deps.cli.kv_list,
        policy = "cli.kv.list",
    })
    app.capability.register({
        surface = "cli",
        name = "kv-get",
        description = "Get a KV entry by key",
        handler = deps.cli.kv_get,
        policy = "cli.kv.get",
    })
    app.capability.register({
        surface = "cli",
        name = "kv-set",
        description = "Set a KV entry (key + value)",
        handler = deps.cli.kv_set,
        policy = "cli.kv.set",
    })
    app.capability.register({
        surface = "cli",
        name = "kv-del",
        description = "Delete a KV entry by key",
        handler = deps.cli.kv_del,
        policy = "cli.kv.delete",
    })
end

return M
