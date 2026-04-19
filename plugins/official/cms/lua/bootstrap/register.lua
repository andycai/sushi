local M = {}

function M.register(deps)
    sushi.api.route("GET", "/api/cms/pages", deps.api.pages_list, { policy = "api.cms.read" })
    sushi.api.route("POST", "/api/cms/pages", deps.api.pages_create, { policy = "api.cms.write" })
    sushi.api.route("PUT", "/api/cms/pages/*", deps.api.pages_update, { policy = "api.cms.write" })
    sushi.api.route("DELETE", "/api/cms/pages/*", deps.api.pages_delete, { policy = "api.cms.delete" })

    sushi.api.route("GET", "/api/cms/posts", deps.api.posts_list, { policy = "api.cms.read" })
    sushi.api.route("POST", "/api/cms/posts", deps.api.posts_create, { policy = "api.cms.write" })
    sushi.api.route("PUT", "/api/cms/posts/*", deps.api.posts_update, { policy = "api.cms.write" })
    sushi.api.route("DELETE", "/api/cms/posts/*", deps.api.posts_delete, { policy = "api.cms.delete" })

    sushi.api.route("GET", "/api/cms/categories", deps.api.categories_list, { policy = "api.cms.read" })
    sushi.api.route("POST", "/api/cms/categories", deps.api.categories_create, { policy = "api.cms.write" })
    sushi.api.route("PUT", "/api/cms/categories/*", deps.api.categories_update, { policy = "api.cms.write" })
    sushi.api.route("DELETE", "/api/cms/categories/*", deps.api.categories_delete, { policy = "api.cms.delete" })

    sushi.api.route("GET", "/app/cms", deps.api.public_home, { public = true })
    sushi.api.route("GET", "/app/pages", deps.api.public_page_list, { public = true })
    sushi.api.route("GET", "/app/pages/*", deps.api.public_page_detail, { public = true })
    sushi.api.route("GET", "/app/posts", deps.api.public_post_list, { public = true })
    sushi.api.route("GET", "/app/partials/cms/posts", deps.api.public_posts_partial, { public = true })
    sushi.api.route("GET", "/app/posts/*", deps.api.public_post_detail, { public = true })
    sushi.api.route("GET", "/app/categories/*", deps.api.public_category_detail, { public = true })
    sushi.api.route("GET", "/admin/preview/cms/pages/*", deps.admin.preview_page_detail, { policy = "admin.cms.read" })
    sushi.api.route("GET", "/admin/preview/cms/posts/*", deps.admin.preview_post_detail, { policy = "admin.cms.read" })

    sushi.api.route("GET", "/admin/partials/cms/pages/table", deps.admin.pages_table_partial, { policy = "admin.cms.read" })
    sushi.api.route("POST", "/admin/partials/cms/pages/upsert", deps.admin.pages_upsert_partial, { policy = "admin.cms.write" })
    sushi.api.route("POST", "/admin/partials/cms/pages/delete", deps.admin.pages_delete_partial, { policy = "admin.cms.write" })
    sushi.api.route("GET", "/admin/partials/cms/posts/table", deps.admin.posts_table_partial, { policy = "admin.cms.read" })
    sushi.api.route("POST", "/admin/partials/cms/posts/upsert", deps.admin.posts_upsert_partial, { policy = "admin.cms.write" })
    sushi.api.route("POST", "/admin/partials/cms/posts/delete", deps.admin.posts_delete_partial, { policy = "admin.cms.write" })
    sushi.api.route("GET", "/admin/partials/cms/categories/table", deps.admin.categories_table_partial, { policy = "admin.cms.read" })
    sushi.api.route("POST", "/admin/partials/cms/categories/upsert", deps.admin.categories_upsert_partial, { policy = "admin.cms.write" })
    sushi.api.route("POST", "/admin/partials/cms/categories/delete", deps.admin.categories_delete_partial, { policy = "admin.cms.write" })

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
