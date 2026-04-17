local M = {}

local function json_ok(status, data)
    return sushi.web.json(status, data)
end

local function json_error(kind, message)
    local status = 500
    if kind == "invalid_input" or kind == "invalid_status" then
        status = 400
    elseif kind == "not_found" then
        status = 404
    elseif kind == "conflict_has_posts" or kind == "conflict" then
        status = 409
    end
    return sushi.web.json(status, { error = tostring(message or kind) })
end

local function strip_query(path)
    return (path or ""):match("^([^%?]+)") or ""
end

local function decode_body(body)
    if not body or body == "" then
        return {}
    end
    local ok, decoded = pcall(function()
        return sushi.json.decode(body)
    end)
    if not ok or type(decoded) ~= "table" then
        return nil, "invalid_input", "invalid json body"
    end
    return decoded
end

function M.new(deps)
    local api = {}
    local page = deps.page
    local post = deps.post
    local category = deps.category
    local markdown = deps.markdown

    function api.pages_list()
        local rows, kind, msg = page.list()
        if not rows then
            return json_error(kind, msg)
        end
        return json_ok(200, { items = rows })
    end

    function api.pages_create(args)
        local payload, kind, msg = decode_body(args[2])
        if not payload then
            return json_error(kind, msg)
        end
        local item, item_kind, item_msg = page.upsert(payload, nil)
        if not item then
            return json_error(item_kind, item_msg)
        end
        return json_ok(201, item)
    end

    function api.pages_update(args)
        local path = strip_query(args[1] or "")
        local current_slug = path:match("^/api/cms/pages/(.+)$")
        if not current_slug then
            return json_error("not_found", "page not found")
        end
        local payload, kind, msg = decode_body(args[2])
        if not payload then
            return json_error(kind, msg)
        end
        local item, item_kind, item_msg = page.upsert(payload, current_slug)
        if not item then
            return json_error(item_kind, item_msg)
        end
        return json_ok(200, item)
    end

    function api.pages_delete(args)
        local path = strip_query(args[1] or "")
        local current_slug = path:match("^/api/cms/pages/(.+)$")
        if not current_slug then
            return json_error("not_found", "page not found")
        end
        local ok, kind, msg = page.soft_delete(current_slug)
        if not ok then
            return json_error(kind, msg)
        end
        return json_ok(200, { ok = true })
    end

    function api.posts_list()
        local rows, kind, msg = post.list({ only_published = false })
        if not rows then
            return json_error(kind, msg)
        end
        return json_ok(200, { items = rows })
    end

    function api.posts_create(args)
        local payload, kind, msg = decode_body(args[2])
        if not payload then
            return json_error(kind, msg)
        end
        local item, item_kind, item_msg = post.upsert(payload, nil)
        if not item then
            return json_error(item_kind, item_msg)
        end
        return json_ok(201, item)
    end

    function api.posts_update(args)
        local path = strip_query(args[1] or "")
        local current_slug = path:match("^/api/cms/posts/(.+)$")
        if not current_slug then
            return json_error("not_found", "post not found")
        end
        local payload, kind, msg = decode_body(args[2])
        if not payload then
            return json_error(kind, msg)
        end
        local item, item_kind, item_msg = post.upsert(payload, current_slug)
        if not item then
            return json_error(item_kind, item_msg)
        end
        return json_ok(200, item)
    end

    function api.posts_delete(args)
        local path = strip_query(args[1] or "")
        local current_slug = path:match("^/api/cms/posts/(.+)$")
        if not current_slug then
            return json_error("not_found", "post not found")
        end
        local ok, kind, msg = post.soft_delete(current_slug)
        if not ok then
            return json_error(kind, msg)
        end
        return json_ok(200, { ok = true })
    end

    function api.categories_list()
        local rows, kind, msg = category.list()
        if not rows then
            return json_error(kind, msg)
        end
        return json_ok(200, { items = rows })
    end

    function api.categories_create(args)
        local payload, kind, msg = decode_body(args[2])
        if not payload then
            return json_error(kind, msg)
        end
        local item, item_kind, item_msg = category.upsert(payload, nil)
        if not item then
            return json_error(item_kind, item_msg)
        end
        return json_ok(201, item)
    end

    function api.categories_update(args)
        local path = strip_query(args[1] or "")
        local current_slug = path:match("^/api/cms/categories/(.+)$")
        if not current_slug then
            return json_error("not_found", "category not found")
        end
        local payload, kind, msg = decode_body(args[2])
        if not payload then
            return json_error(kind, msg)
        end
        local item, item_kind, item_msg = category.upsert(payload, current_slug)
        if not item then
            return json_error(item_kind, item_msg)
        end
        return json_ok(200, item)
    end

    function api.categories_delete(args)
        local path = strip_query(args[1] or "")
        local current_slug = path:match("^/api/cms/categories/(.+)$")
        if not current_slug then
            return json_error("not_found", "category not found")
        end
        local ok, kind, msg = category.soft_delete(current_slug)
        if not ok then
            return json_error(kind, msg)
        end
        return json_ok(200, { ok = true })
    end

    function api.public_page_detail(args)
        local path = strip_query(args[1] or "")
        local slug = path:match("^/app/pages/([^%?]+)$")
        if not slug then
            return json_error("not_found", "not found")
        end
        local item, kind, msg = page.get_by_slug(slug, { only_published = true })
        if not item then
            return json_error(kind, msg)
        end
        return sushi.web.render("plugins/official/cms/public/page_detail.html", {
            title = item.title,
            content_html = markdown.to_html(item.markdown_body),
        })
    end

    function api.public_post_list(args)
        local path = args[1] or ""
        local category_slug = path:match("[?&]category=([^&]+)")
        local rows, kind, msg = post.list({ only_published = true, category_slug = category_slug })
        if not rows then
            return json_error(kind, msg)
        end
        return sushi.web.render("plugins/official/cms/public/post_list.html", {
            items = rows,
            category = category_slug,
        })
    end

    function api.public_post_detail(args)
        local path = strip_query(args[1] or "")
        local slug = path:match("^/app/posts/([^%?]+)$")
        if not slug then
            return json_error("not_found", "not found")
        end
        local item, kind, msg = post.get_by_slug(slug, { only_published = true })
        if not item then
            return json_error(kind, msg)
        end
        return sushi.web.render("plugins/official/cms/public/post_detail.html", {
            title = item.title,
            category_name = item.category_name or "",
            content_html = markdown.to_html(item.markdown_body),
        })
    end

    function api.public_category_detail(args)
        local path = strip_query(args[1] or "")
        local slug = path:match("^/app/categories/([^%?]+)$")
        if not slug then
            return json_error("not_found", "not found")
        end
        local item, kind, msg = category.get_by_slug(slug)
        if not item then
            return json_error(kind, msg)
        end
        local rows, list_kind, list_msg =
            post.list({ only_published = true, category_slug = slug })
        if not rows then
            return json_error(list_kind, list_msg)
        end
        return sushi.web.render("plugins/official/cms/public/category_detail.html", {
            category = item,
            items = rows,
        })
    end

    return api
end

return M
