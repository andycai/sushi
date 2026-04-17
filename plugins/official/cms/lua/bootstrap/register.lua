local M = {}

function M.register(deps)
    sushi.api.route("GET", "/api/cms/pages", deps.api.pages_list, { policy = "api.cms.pages.read" })
    sushi.api.route("POST", "/api/cms/pages", deps.api.pages_create, { policy = "api.cms.pages.write" })
    sushi.api.route("PUT", "/api/cms/pages/*", deps.api.pages_update, { policy = "api.cms.pages.write" })
    sushi.api.route("DELETE", "/api/cms/pages/*", deps.api.pages_delete, { policy = "api.cms.pages.delete" })

    sushi.api.route("GET", "/api/cms/posts", deps.api.posts_list, { policy = "api.cms.posts.read" })
    sushi.api.route("POST", "/api/cms/posts", deps.api.posts_create, { policy = "api.cms.posts.write" })
    sushi.api.route("PUT", "/api/cms/posts/*", deps.api.posts_update, { policy = "api.cms.posts.write" })
    sushi.api.route("DELETE", "/api/cms/posts/*", deps.api.posts_delete, { policy = "api.cms.posts.delete" })

    sushi.api.route("GET", "/api/cms/categories", deps.api.categories_list, { policy = "api.cms.categories.read" })
    sushi.api.route("POST", "/api/cms/categories", deps.api.categories_create, { policy = "api.cms.categories.write" })
    sushi.api.route("PUT", "/api/cms/categories/*", deps.api.categories_update, { policy = "api.cms.categories.write" })
    sushi.api.route("DELETE", "/api/cms/categories/*", deps.api.categories_delete, { policy = "api.cms.categories.delete" })

    sushi.api.route("GET", "/app/pages/*", deps.api.public_page_detail, { policy = "api.cms.public.read" })
    sushi.api.route("GET", "/app/posts", deps.api.public_post_list, { policy = "api.cms.public.read" })
    sushi.api.route("GET", "/app/posts/*", deps.api.public_post_detail, { policy = "api.cms.public.read" })
    sushi.api.route("GET", "/app/categories/*", deps.api.public_category_detail, { policy = "api.cms.public.read" })

    sushi.api.route("GET", "/admin/partials/cms/overview", deps.admin.overview_partial, { policy = "admin.cms.read" })
    sushi.api.route("GET", "/admin/partials/cms/library/*", deps.admin.library_partial, { policy = "admin.cms.read" })
    sushi.api.route("GET", "/admin/partials/cms/editor/*", deps.admin.editor_partial, { policy = "admin.cms.read" })
    sushi.api.route("POST", "/admin/partials/cms/editor/save", deps.admin.editor_save_partial, { policy = "admin.cms.write" })
    sushi.api.route("POST", "/admin/partials/cms/status/transition", deps.admin.status_transition_partial, { policy = "admin.cms.write" })
    sushi.api.route("GET", "/admin/partials/cms/commands", deps.admin.commands_partial, { policy = "admin.cms.read" })

    sushi.web.page("/admin/cms", "plugins/official/cms/cms.html", {
        title = "CMS",
        policy = "admin.cms.read",
        assets = { bundles = { "workspace" } },
    })

    sushi.cli.command("cms", "CMS CRUD command", deps.cli.cms_dispatch, { policy = "cli.cms.execute" })
end

return M
