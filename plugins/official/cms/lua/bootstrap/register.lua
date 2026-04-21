local M = {}

local function register_api_route(definition)
    definition.surface = "api"
    sushi.capability.register(definition)
end

local function register_web_page(definition)
    definition.surface = "web"
    definition.kind = "page"
    sushi.capability.register(definition)
end

local function register_cli_command(definition)
    definition.surface = "cli"
    sushi.capability.register(definition)
end

function M.register(deps)
    register_api_route({ method = "GET", path = "/api/cms/pages", handler = deps.api.pages_list, policy = "api.cms.read" })
    register_api_route({ method = "POST", path = "/api/cms/pages", handler = deps.api.pages_create, policy = "api.cms.write" })
    register_api_route({ method = "PUT", path = "/api/cms/pages/*", handler = deps.api.pages_update, policy = "api.cms.write" })
    register_api_route({
        method = "DELETE",
        path = "/api/cms/pages/*",
        handler = deps.api.pages_delete,
        policy = "api.cms.delete",
    })

    register_api_route({ method = "GET", path = "/api/cms/posts", handler = deps.api.posts_list, policy = "api.cms.read" })
    register_api_route({ method = "POST", path = "/api/cms/posts", handler = deps.api.posts_create, policy = "api.cms.write" })
    register_api_route({ method = "PUT", path = "/api/cms/posts/*", handler = deps.api.posts_update, policy = "api.cms.write" })
    register_api_route({
        method = "DELETE",
        path = "/api/cms/posts/*",
        handler = deps.api.posts_delete,
        policy = "api.cms.delete",
    })

    register_api_route({
        method = "GET",
        path = "/api/cms/categories",
        handler = deps.api.categories_list,
        policy = "api.cms.read",
    })
    register_api_route({
        method = "POST",
        path = "/api/cms/categories",
        handler = deps.api.categories_create,
        policy = "api.cms.write",
    })
    register_api_route({
        method = "PUT",
        path = "/api/cms/categories/*",
        handler = deps.api.categories_update,
        policy = "api.cms.write",
    })
    register_api_route({
        method = "DELETE",
        path = "/api/cms/categories/*",
        handler = deps.api.categories_delete,
        policy = "api.cms.delete",
    })

    register_api_route({ method = "GET", path = "/app/cms", handler = deps.api.public_home, public = true })
    register_api_route({ method = "GET", path = "/app/pages", handler = deps.api.public_page_list, public = true })
    register_api_route({ method = "GET", path = "/app/pages/*", handler = deps.api.public_page_detail, public = true })
    register_api_route({ method = "GET", path = "/app/posts", handler = deps.api.public_post_list, public = true })
    register_api_route({
        method = "GET",
        path = "/app/partials/cms/posts",
        handler = deps.api.public_posts_partial,
        public = true,
    })
    register_api_route({ method = "GET", path = "/app/posts/*", handler = deps.api.public_post_detail, public = true })
    register_api_route({
        method = "GET",
        path = "/app/categories/*",
        handler = deps.api.public_category_detail,
        public = true,
    })
    register_api_route({
        method = "GET",
        path = "/admin/preview/cms/pages/*",
        handler = deps.admin.preview_page_detail,
        policy = "admin.cms.read",
    })
    register_api_route({
        method = "GET",
        path = "/admin/preview/cms/posts/*",
        handler = deps.admin.preview_post_detail,
        policy = "admin.cms.read",
    })

    register_api_route({
        method = "GET",
        path = "/admin/partials/cms/pages/table",
        handler = deps.admin.pages_table_partial,
        policy = "admin.cms.read",
    })
    register_api_route({
        method = "POST",
        path = "/admin/partials/cms/pages/upsert",
        handler = deps.admin.pages_upsert_partial,
        policy = "admin.cms.write",
    })
    register_api_route({
        method = "POST",
        path = "/admin/partials/cms/pages/delete",
        handler = deps.admin.pages_delete_partial,
        policy = "admin.cms.write",
    })
    register_api_route({
        method = "GET",
        path = "/admin/partials/cms/posts/table",
        handler = deps.admin.posts_table_partial,
        policy = "admin.cms.read",
    })
    register_api_route({
        method = "POST",
        path = "/admin/partials/cms/posts/upsert",
        handler = deps.admin.posts_upsert_partial,
        policy = "admin.cms.write",
    })
    register_api_route({
        method = "POST",
        path = "/admin/partials/cms/posts/delete",
        handler = deps.admin.posts_delete_partial,
        policy = "admin.cms.write",
    })
    register_api_route({
        method = "GET",
        path = "/admin/partials/cms/categories/table",
        handler = deps.admin.categories_table_partial,
        policy = "admin.cms.read",
    })
    register_api_route({
        method = "POST",
        path = "/admin/partials/cms/categories/upsert",
        handler = deps.admin.categories_upsert_partial,
        policy = "admin.cms.write",
    })
    register_api_route({
        method = "POST",
        path = "/admin/partials/cms/categories/delete",
        handler = deps.admin.categories_delete_partial,
        policy = "admin.cms.write",
    })

    register_api_route({
        method = "GET",
        path = "/admin/partials/cms/overview",
        handler = deps.admin.overview_partial,
        policy = "admin.cms.read",
    })
    register_api_route({
        method = "GET",
        path = "/admin/partials/cms/library/*",
        handler = deps.admin.library_partial,
        policy = "admin.cms.read",
    })
    register_api_route({
        method = "GET",
        path = "/admin/partials/cms/editor/*",
        handler = deps.admin.editor_partial,
        policy = "admin.cms.read",
    })
    register_api_route({
        method = "POST",
        path = "/admin/partials/cms/editor/save",
        handler = deps.admin.editor_save_partial,
        policy = "admin.cms.write",
    })
    register_api_route({
        method = "POST",
        path = "/admin/partials/cms/status/transition",
        handler = deps.admin.status_transition_partial,
        policy = "admin.cms.write",
    })
    register_api_route({
        method = "GET",
        path = "/admin/partials/cms/commands",
        handler = deps.admin.commands_partial,
        policy = "admin.cms.read",
    })

    register_web_page({
        path = "/admin/cms",
        template = "plugins/official/cms/cms.html",
        title = "CMS",
        policy = "admin.cms.read",
        assets = { bundles = { "workspace" } },
        handler = function()
            return sushi.web.render("plugins/official/cms/cms.html")
        end,
    })

    register_cli_command({
        name = "cms",
        description = "CMS CRUD command",
        handler = deps.cli.cms_dispatch,
        policy = "cli.cms.execute",
    })
end

return M
