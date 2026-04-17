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

    return admin
end

return M
