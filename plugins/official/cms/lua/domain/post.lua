local M = {}

function M.new(deps)
    local db = deps.db
    local validate = deps.validate
    local slug = deps.slug
    local post = {}

    local function resolve_category_id(category_slug)
        local rows, kind, msg = db.query(
            "SELECT id FROM cms_categories WHERE slug = ?1 AND deleted_at IS NULL LIMIT 1",
            { category_slug }
        )
        if not rows then
            return nil, kind or "storage_error", msg
        end
        if #rows == 0 then
            return nil, "not_found", "category not found"
        end
        return rows[1].id
    end

    function post.list(opts)
        local only_published = opts and opts.only_published
        local where = "p.deleted_at IS NULL AND c.deleted_at IS NULL"
        local params = {}
        if only_published then
            where = where .. " AND p.status = 'published'"
        end
        if opts and opts.category_slug and opts.category_slug ~= "" then
            where = where .. " AND c.slug = ?1"
            params = { opts.category_slug }
        end
        local rows, kind, msg = db.query(
            "SELECT p.id, p.title, p.slug, p.excerpt, p.markdown_body, p.status, c.slug AS category_slug, c.name AS category_name "
                .. "FROM cms_posts p JOIN cms_categories c ON c.id = p.category_id "
                .. "WHERE "
                .. where
                .. " ORDER BY p.updated_at DESC",
            params
        )
        if not rows then
            return nil, kind or "storage_error", msg
        end
        return rows
    end

    function post.get_by_slug(value, opts)
        local only_published = opts and opts.only_published
        local sql = "SELECT p.id, p.title, p.slug, p.excerpt, p.markdown_body, p.status, c.slug AS category_slug, c.name AS category_name "
            .. "FROM cms_posts p JOIN cms_categories c ON c.id = p.category_id "
            .. "WHERE p.slug = ?1 AND p.deleted_at IS NULL AND c.deleted_at IS NULL"
        if only_published then
            sql = sql .. " AND p.status = 'published'"
        end
        local rows, kind, msg = db.query(sql, { value })
        if not rows then
            return nil, kind or "storage_error", msg
        end
        if #rows == 0 then
            return nil, "not_found", "post not found"
        end
        return rows[1]
    end

    function post.upsert(payload, original_slug)
        local title, title_kind, title_msg = validate.require_non_empty(payload.title, "title")
        if not title then
            return nil, title_kind, title_msg
        end
        local slug_input, slug_kind, slug_msg = validate.require_non_empty(payload.slug, "slug")
        if not slug_input then
            return nil, slug_kind, slug_msg
        end
        local normalized_slug = slug.normalize(slug_input)
        if normalized_slug == "" then
            return nil, "invalid_slug", "slug cannot be empty"
        end
        local body, body_kind, body_msg =
            validate.require_non_empty(payload.markdown_body, "markdown_body")
        if not body then
            return nil, body_kind, body_msg
        end
        local status, status_kind, status_msg =
            validate.validate_status(payload.status or "draft")
        if not status then
            return nil, status_kind, status_msg
        end
        local category_slug, cat_kind, cat_msg =
            validate.require_non_empty(payload.category_slug, "category_slug")
        if not category_slug then
            return nil, cat_kind, cat_msg
        end
        local category_id, id_kind, id_msg = resolve_category_id(category_slug)
        if not category_id then
            return nil, id_kind, id_msg
        end
        local excerpt = payload.excerpt

        local ok, exec_kind, exec_msg
        if original_slug and original_slug ~= "" then
            ok, exec_kind, exec_msg = db.execute(
                "UPDATE cms_posts SET title = ?1, slug = ?2, excerpt = ?3, markdown_body = ?4, status = ?5, category_id = ?6, updated_at = datetime('now') WHERE slug = ?7 AND deleted_at IS NULL",
                { title, normalized_slug, excerpt, body, status, category_id, original_slug }
            )
        else
            ok, exec_kind, exec_msg = db.execute(
                "INSERT INTO cms_posts (title, slug, excerpt, markdown_body, status, category_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                { title, normalized_slug, excerpt, body, status, category_id }
            )
        end
        if not ok then
            return nil, exec_kind or "storage_error", exec_msg
        end

        return post.get_by_slug(normalized_slug)
    end

    function post.soft_delete(value)
        local ok, kind, msg = db.execute(
            "UPDATE cms_posts SET deleted_at = datetime('now'), updated_at = datetime('now') WHERE slug = ?1 AND deleted_at IS NULL",
            { value }
        )
        if not ok then
            return nil, kind or "storage_error", msg
        end
        return true
    end

    return post
end

return M
