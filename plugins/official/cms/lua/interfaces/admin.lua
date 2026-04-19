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

local function request_path(args)
    return strip_query((args and args.dispatch_path) or (args and args[1]) or "")
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
        local path = request_path(args)
        local from_path = path:match("^" .. route_prefix .. "/([^/]+)$")
        if from_path then
            return normalize_resource(from_path)
        end
        local form = parse_urlencoded((args and args[2]) or "")
        return normalize_resource(form.resource or form.content_type or form.kind or form.type)
    end

    local function parse_query_params(path)
        local out = {}
        local query = tostring(path or ""):match("%?(.*)$")
        if not query then
            return out
        end
        for pair in query:gmatch("[^&]+") do
            local key, value = pair:match("([^=]+)=(.*)")
            if key then
                out[url_decode(key)] = url_decode(value)
            end
        end
        return out
    end

    local function list_scope_rows(resource)
        if resource == "pages" then
            return page.list()
        end
        if resource == "posts" then
            return post.list({ only_published = false })
        end
        if resource == "categories" then
            return category.list()
        end
        return nil, "invalid_resource", "Unknown CMS resource"
    end

    local function count_summary(rows)
        local summary = {
            draft = 0,
            published = 0,
            archived = 0,
            total = 0,
        }
        for _, row in ipairs(rows or {}) do
            local status = tostring(row.status or ""):lower()
            local total = tonumber(row.total) or 0
            if status == "draft" then
                summary.draft = total
            elseif status == "published" then
                summary.published = total
            elseif status == "archived" then
                summary.archived = total
            end
            summary.total = summary.total + total
        end
        return summary
    end

    local function render_library_panel(scope, path)
        local rows, kind, msg = list_scope_rows(scope)
        if not rows then
            return flash("error", tostring(msg or kind or "failed to load library"))
        end
        local params = parse_query_params(path)
        return sushi.web.render("plugins/official/cms/fragments/library_panel.html", {
            scope = scope,
            items = rows,
            query = params.q or "",
        })
    end

    local function resolve_editor_resource(args, form)
        local path = request_path(args)
        local from_path = path:match("^/admin/partials/cms/editor/([^/?]+)")
        if from_path then
            return normalize_resource(from_path)
        end

        local kind = tostring(form.kind or ""):lower()
        local resource = tostring(form.resource or form.content_type or form.type or ""):lower()

        if kind == "page" or resource == "page" then
            return "pages"
        end
        if kind == "post" or resource == "post" then
            return "posts"
        end

        return normalize_resource(kind) or normalize_resource(resource)
    end

    local function resolve_editor_slug(args, form)
        local path = request_path(args)
        local from_path = path:match("^/admin/partials/cms/editor/[^/?]+/([^/?]+)")
        if from_path and from_path ~= "" then
            return from_path
        end
        local from_form = tostring(form.slug or form.target_slug or form.original_slug or "")
        if from_form ~= "" then
            return from_form
        end
        return "new"
    end

    local function editor_item_defaults(resource)
        if resource == "categories" then
            return {
                slug = "",
                name = "",
                description = "",
            }
        end
        if resource == "pages" then
            return {
                slug = "",
                title = "",
                status = "draft",
                markdown_body = "",
            }
        end
        return {
            slug = "",
            title = "",
            status = "draft",
            excerpt = "",
            markdown_body = "",
            category_slug = "",
        }
    end

    local function load_editor_item(resource, slug_value)
        if slug_value == "new" then
            return editor_item_defaults(resource), "create"
        end
        if resource == "pages" then
            local item, kind, msg = page.get_by_slug(slug_value, { only_published = false })
            if not item then
                return nil, nil, kind, msg
            end
            return item, "edit"
        end
        if resource == "posts" then
            local item, kind, msg = post.get_by_slug(slug_value, { only_published = false })
            if not item then
                return nil, nil, kind, msg
            end
            return item, "edit"
        end
        if resource == "categories" then
            local item, kind, msg = category.get_by_slug(slug_value)
            if not item then
                return nil, nil, kind, msg
            end
            return item, "edit"
        end
        return nil, nil, "invalid_resource", "Unknown CMS resource"
    end

    local function editor_title(resource, mode)
        if resource == "pages" then
            return mode == "edit" and "Edit Page" or "New Page"
        end
        if resource == "categories" then
            return mode == "edit" and "Edit Category" or "New Category"
        end
        return mode == "edit" and "Edit Post" or "New Post"
    end

    local function render_editor_panel(resource, slug_value)
        local item, mode, kind, msg = load_editor_item(resource, slug_value)
        if not item then
            return flash("error", tostring(msg or kind or "failed to load editor"))
        end
        local categories = {}
        if resource == "posts" then
            local rows, cat_kind, cat_msg = category.list()
            if not rows then
                return flash("error", tostring(cat_msg or cat_kind or "failed to load categories"))
            end
            categories = rows
        end
        return sushi.web.render("plugins/official/cms/fragments/editor_panel.html", {
            resource = resource,
            item = item,
            categories = categories,
            mode = mode,
            editor_title = editor_title(resource, mode),
        })
    end

    local function save_from_editor(resource, form)
        if resource == "pages" then
            local item, kind, msg = page.upsert({
                title = form.title,
                slug = form.slug,
                markdown_body = form.markdown_body,
                status = form.status,
            }, form.original_slug)
            if not item then
                return flash("error", tostring(msg or kind or "failed to save page"))
            end
            return flash("success", "Page saved")
        end
        if resource == "posts" then
            local item, kind, msg = post.upsert({
                title = form.title,
                slug = form.slug,
                excerpt = form.excerpt,
                markdown_body = form.markdown_body,
                status = form.status,
                category_slug = form.category_slug,
            }, form.original_slug)
            if not item then
                return flash("error", tostring(msg or kind or "failed to save post"))
            end
            return flash("success", "Post saved")
        end
        if resource == "categories" then
            local item, kind, msg = category.upsert({
                name = form.name,
                slug = form.slug,
                description = form.description,
            }, form.original_slug)
            if not item then
                return flash("error", tostring(msg or kind or "failed to save category"))
            end
            return flash("success", "Category saved")
        end
        return flash("error", "Missing CMS resource for editor save")
    end

    local function render_overview_panel(data)
        -- cms_overview_template_fallback_marker
        local ok, html = pcall(sushi.web.render, "plugins/official/cms/fragments/overview_panel.html", data)
        if ok and html then
            return html
        end
        return sushi.web.render("plugins/official/cms/fragments/rows.html", {
            label = "Overview panel template is not available yet.",
        })
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
        local page_counts_raw, page_kind, page_msg = page.count_by_status()
        if not page_counts_raw then
            return flash("error", tostring(page_msg or page_kind or "failed to load page overview"))
        end
        local post_counts_raw, post_kind, post_msg = post.count_by_status()
        if not post_counts_raw then
            return flash("error", tostring(post_msg or post_kind or "failed to load post overview"))
        end
        local recent_pages, pages_recent_kind, pages_recent_msg = page.recent(5)
        if not recent_pages then
            return flash("error", tostring(pages_recent_msg or pages_recent_kind or "failed to load recent pages"))
        end
        local recent_posts, posts_recent_kind, posts_recent_msg = post.recent(5)
        if not recent_posts then
            return flash("error", tostring(posts_recent_msg or posts_recent_kind or "failed to load recent posts"))
        end

        return render_overview_panel({
            page_counts = count_summary(page_counts_raw),
            post_counts = count_summary(post_counts_raw),
            recent_pages = recent_pages,
            recent_posts = recent_posts,
        })
    end

    function admin.library_partial(args)
        local path = request_path(args)
        local scope = path:match("^/admin/partials/cms/library/([^/?]+)")
        if scope then
            local normalized = normalize_resource(scope)
            if normalized then
                return render_library_panel(normalized, path)
            end
            return flash("error", "Unknown CMS resource")
        end
        local resource = resolve_resource(args, "/admin/partials/cms/library")
        if resource then
            return render_library_panel(resource, path)
        end
        return render_library_panel("posts", path)
    end

    function admin.editor_partial(args)
        local form = parse_urlencoded((args and args[2]) or "")
        local resource = resolve_editor_resource(args, form)
        if resource then
            local slug_value = resolve_editor_slug(args, form)
            return render_editor_panel(resource, slug_value)
        end
        return flash("info", "Workbench editor is ready; select pages, posts, or categories.")
    end

    function admin.editor_save_partial(args)
        local form = parse_urlencoded((args and args[2]) or "")
        local resource = resolve_editor_resource(args, form)
        if resource then
            return save_from_editor(resource, form)
        end
        return flash("error", "Missing CMS resource for editor save")
    end

    function admin.status_transition_partial(args)
        local form = parse_urlencoded((args and args[2]) or "")
        local path = request_path(args)
        local resource = normalize_resource(path:match("^/admin/partials/cms/status/([^/?]+)") or form.resource or form.content_type or form.kind or form.type)
        local slug_value = form.slug or form.target_slug
        local next_status = form.status or form.next_status

        if resource == "pages" then
            local item, kind, msg = page.set_status(slug_value, next_status)
            if not item then
                return flash("error", tostring(msg or kind or "failed to change page status"))
            end
            return flash("success", "Page status updated")
        end
        if resource == "posts" then
            local item, kind, msg = post.set_status(slug_value, next_status)
            if not item then
                return flash("error", tostring(msg or kind or "failed to change post status"))
            end
            return flash("success", "Post status updated")
        end
        if resource == "categories" then
            return flash("error", "Categories do not support status transitions")
        end
        return flash("error", "Missing CMS resource for status transition")
    end

    function admin.commands_partial()
        return sushi.web.render("plugins/official/cms/fragments/rows.html", {
            label = "Use `sushi cms page list` or `sushi cms post list` to inspect content from CLI.",
        })
    end

    return admin
end

return M
