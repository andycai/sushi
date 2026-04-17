local M = {}

local function url_decode(text)
    local value = tostring(text or "")
    value = value:gsub("+", " ")
    value = value:gsub("%%(%x%x)", function(hex)
        return string.char(tonumber(hex, 16))
    end)
    return value
end

local function parse_urlencoded(body)
    local out = {}
    for pair in tostring(body or ""):gmatch("[^&]+") do
        local key, value = pair:match("([^=]*)=(.*)")
        if key then
            out[url_decode(key)] = url_decode(value)
        end
    end
    return out
end

local function strip_query(path)
    return (path or ""):match("^([^%?]+)") or ""
end

function M.new(deps)
    local admin = {}
    local page = deps.page
    local post = deps.post
    local category = deps.category

    local function flash(level, message)
        return sushi.web.render("plugins/official/cms/fragments/flash.html", {
            level = tostring(level or "success"),
            message = tostring(message or ""),
        })
    end

    local function normalize_resource(resource)
        local value = tostring(resource or ""):lower()
        if value == "page" or value == "pages" then
            return "pages"
        end
        if value == "post" or value == "posts" then
            return "posts"
        end
        if value == "category" or value == "categories" then
            return "categories"
        end
        return nil
    end

    local function resolve_resource(args, route_prefix)
        local path = strip_query((args and args[1]) or "")
        local from_path = path:match("^" .. route_prefix .. "/([^/]+)$")
        if from_path then
            return normalize_resource(from_path)
        end
        local form = parse_urlencoded((args and args[2]) or "")
        return normalize_resource(form.resource or form.content_type or form.kind or form.type)
    end

    local function dispatch_table_partial(resource)
        if resource == "pages" then
            return admin.pages_table_partial()
        end
        if resource == "posts" then
            return admin.posts_table_partial()
        end
        if resource == "categories" then
            return admin.categories_table_partial()
        end
        return flash("error", "Unknown CMS resource")
    end

    local function dispatch_upsert_partial(resource, args)
        if resource == "pages" then
            return admin.pages_upsert_partial(args)
        end
        if resource == "posts" then
            return admin.posts_upsert_partial(args)
        end
        if resource == "categories" then
            return admin.categories_upsert_partial(args)
        end
        return flash("error", "Unknown CMS resource")
    end

    function admin.pages_table_partial()
        local rows, _, _ = page.list()
        return sushi.web.render("plugins/official/cms/fragments/page_rows.html", {
            items = rows or {},
        })
    end

    function admin.pages_upsert_partial(args)
        local form = parse_urlencoded(args[2] or "")
        local payload = {
            title = form.title,
            slug = form.slug,
            markdown_body = form.markdown_body,
            status = form.status,
        }
        local item, kind, msg = page.upsert(payload, form.original_slug)
        if not item then
            return flash("error", tostring(msg or kind or "failed to save page"))
        end
        return flash("success", "Page saved")
    end

    function admin.pages_delete_partial(args)
        local form = parse_urlencoded(args[2] or "")
        local ok, kind, msg = page.soft_delete(form.slug or "")
        if not ok then
            return flash("error", tostring(msg or kind or "failed to delete page"))
        end
        return flash("success", "Page deleted")
    end

    function admin.posts_table_partial()
        local rows, _, _ = post.list({ only_published = false })
        return sushi.web.render("plugins/official/cms/fragments/post_rows.html", {
            items = rows or {},
        })
    end

    function admin.posts_upsert_partial(args)
        local form = parse_urlencoded(args[2] or "")
        local payload = {
            title = form.title,
            slug = form.slug,
            excerpt = form.excerpt,
            markdown_body = form.markdown_body,
            status = form.status,
            category_slug = form.category_slug,
        }
        local item, kind, msg = post.upsert(payload, form.original_slug)
        if not item then
            return flash("error", tostring(msg or kind or "failed to save post"))
        end
        return flash("success", "Post saved")
    end

    function admin.posts_delete_partial(args)
        local form = parse_urlencoded(args[2] or "")
        local ok, kind, msg = post.soft_delete(form.slug or "")
        if not ok then
            return flash("error", tostring(msg or kind or "failed to delete post"))
        end
        return flash("success", "Post deleted")
    end

    function admin.categories_table_partial()
        local rows, _, _ = category.list()
        return sushi.web.render("plugins/official/cms/fragments/category_rows.html", {
            items = rows or {},
        })
    end

    function admin.categories_upsert_partial(args)
        local form = parse_urlencoded(args[2] or "")
        local payload = {
            name = form.name,
            slug = form.slug,
            description = form.description,
        }
        local item, kind, msg = category.upsert(payload, form.original_slug)
        if not item then
            return flash("error", tostring(msg or kind or "failed to save category"))
        end
        return flash("success", "Category saved")
    end

    function admin.categories_delete_partial(args)
        local form = parse_urlencoded(args[2] or "")
        local ok, kind, msg = category.soft_delete(form.slug or "")
        if not ok then
            if kind == "conflict_has_posts" then
                return flash("error", "Category still has posts and cannot be deleted")
            end
            return flash("error", tostring(msg or kind or "failed to delete category"))
        end
        return flash("success", "Category deleted")
    end

    function admin.overview_partial()
        return admin.pages_table_partial()
    end

    function admin.library_partial(args)
        local resource = resolve_resource(args, "/admin/partials/cms/library")
        if resource then
            return dispatch_table_partial(resource)
        end
        return admin.pages_table_partial()
    end

    function admin.editor_partial(args)
        local resource = resolve_resource(args, "/admin/partials/cms/editor")
        if resource then
            return dispatch_table_partial(resource)
        end
        return flash("info", "Workbench editor bridge is active; use current CMS forms.")
    end

    function admin.editor_save_partial(args)
        local resource = resolve_resource(args, "/admin/partials/cms/editor")
        if resource then
            return dispatch_upsert_partial(resource, args)
        end
        return flash("error", "Missing CMS resource for editor save")
    end

    function admin.status_transition_partial(args)
        local resource = resolve_resource(args, "/admin/partials/cms/status")
        if resource then
            return dispatch_upsert_partial(resource, args)
        end
        return flash("error", "Missing CMS resource for status transition")
    end

    function admin.commands_partial()
        return flash("info", "Workbench commands panel will be introduced in a follow-up task.")
    end

    return admin
end

return M
