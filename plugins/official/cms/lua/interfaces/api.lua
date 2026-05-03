local M = {}

local function json_ok(status, data)
    return app.web.json(status, data)
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
    return app.web.json(status, { error = tostring(message or kind) })
end

local function strip_query(path)
    return (path or ""):match("^([^%?]+)") or ""
end

local function request_path(args)
    return (args and args.dispatch_path) or (args and args[1]) or ""
end

local function url_decode(text)
    local value = tostring(text or "")
    value = value:gsub("+", " ")
    value = value:gsub("%%(%x%x)", function(hex)
        return string.char(tonumber(hex, 16))
    end)
    return value
end

local function parse_query_params(path)
    local params = {}
    local query = tostring(path or ""):match("%?(.*)$")
    if not query then
        return params
    end
    for pair in query:gmatch("[^&]+") do
        local key, value = pair:match("([^=]+)=(.*)")
        if key then
            params[url_decode(key)] = url_decode(value)
        else
            params[url_decode(pair)] = ""
        end
    end
    return params
end

local function take_limit(rows, limit)
    local out = {}
    if type(rows) ~= "table" then
        return out
    end
    local max = tonumber(limit) or 0
    if max < 1 then
        return out
    end
    for i, row in ipairs(rows) do
        if i > max then
            break
        end
        out[#out + 1] = row
    end
    return out
end

local function published_pages_only(rows)
    local out = {}
    for _, row in ipairs(rows or {}) do
        if tostring(row.status or "") == "published" then
            out[#out + 1] = row
        end
    end
    return out
end

local function parse_positive_limit(raw, hard_max)
    local value = tostring(raw or "")
    if value == "" then
        return nil
    end
    if not value:match("^%d+$") then
        return nil
    end
    local parsed = tonumber(value)
    if not parsed or parsed < 1 then
        return nil
    end
    if hard_max and parsed > hard_max then
        return hard_max
    end
    return parsed
end

local function without_slug(rows, excluded_slug)
    local slug = tostring(excluded_slug or "")
    if slug == "" then
        return rows
    end
    local out = {}
    for _, row in ipairs(rows or {}) do
        if tostring(row.slug or "") ~= slug then
            out[#out + 1] = row
        end
    end
    return out
end

local function decode_body(body)
    if not body or body == "" then
        return {}
    end
    local ok, decoded = pcall(function()
        return app.json.decode(body)
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

    function api.public_page_list()
        local rows, kind, msg = page.list()
        if not rows then
            return json_error(kind, msg)
        end
        local published_pages = published_pages_only(rows)
        return app.web.render("plugins/official/cms/public/page_list.html", {
            items = published_pages,
            total = #published_pages,
        })
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
        return app.web.render("plugins/official/cms/public/page_detail.html", {
            title = item.title,
            slug = item.slug,
            content_html = markdown.to_html(item.markdown_body),
        })
    end

    function api.public_home()
        local post_rows, post_kind, post_msg = post.list({ only_published = true })
        if not post_rows then
            return json_error(post_kind, post_msg)
        end
        local page_rows, page_kind, page_msg = page.list()
        if not page_rows then
            return json_error(page_kind, page_msg)
        end
        local category_rows, category_kind, category_msg = category.list()
        if not category_rows then
            return json_error(category_kind, category_msg)
        end

        local published_pages = published_pages_only(page_rows)
        local featured_post = post_rows[1]
        local featured_page = published_pages[1]

        return app.web.render("plugins/official/cms/public/home.html", {
            featured_post = featured_post,
            featured_page = featured_page,
            recent_posts = take_limit(post_rows, 6),
            published_pages = take_limit(published_pages, 6),
            categories = take_limit(category_rows, 8),
            total_posts = #post_rows,
            total_pages = #published_pages,
            total_categories = #category_rows,
        })
    end

    function api.public_post_list(args)
        local path = request_path(args)
        local params = parse_query_params(path)
        local category_slug = tostring(params.category or "")
        if category_slug == "" then
            category_slug = nil
        end
        local rows, kind, msg = post.list({ only_published = true, category_slug = category_slug })
        if not rows then
            return json_error(kind, msg)
        end
        local category_rows, category_kind, category_msg = category.list()
        if not category_rows then
            return json_error(category_kind, category_msg)
        end
        return app.web.render("plugins/official/cms/public/post_list.html", {
            items = rows,
            category = category_slug or "",
            categories = category_rows,
            total = #rows,
        })
    end

    function api.public_posts_partial(args)
        local path = request_path(args)
        local params = parse_query_params(path)
        local category_slug = tostring(params.category or "")
        if category_slug == "" then
            category_slug = nil
        end
        local rows, kind, msg = post.list({ only_published = true, category_slug = category_slug })
        if not rows then
            return json_error(kind, msg)
        end
        rows = without_slug(rows, params.exclude)
        local limit = parse_positive_limit(params.limit, 24)
        if limit then
            rows = take_limit(rows, limit)
        end
        return app.web.render("plugins/official/cms/public/partials/post_feed.html", {
            items = rows,
            compact = tostring(params.compact or "") == "1",
            show_category = tostring(params.show_category or "") == "1",
            empty_message = "No posts found.",
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
        return app.web.render("plugins/official/cms/public/post_detail.html", {
            title = item.title,
            slug = item.slug,
            category_name = item.category_name or "",
            category_slug = item.category_slug or "",
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
        return app.web.render("plugins/official/cms/public/category_detail.html", {
            category = item,
            items = rows,
            total = #rows,
        })
    end

    return api
end

return M
