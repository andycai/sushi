local M = {}

function M.new(deps)
    local db = deps.db
    local validate = deps.validate
    local slug = deps.slug
    local page = {}

    function page.list()
        local rows, kind, msg = db.query(
            "SELECT id, title, slug, status, created_at, updated_at FROM cms_pages WHERE deleted_at IS NULL ORDER BY updated_at DESC",
            {}
        )
        if not rows then
            return nil, kind or "storage_error", msg
        end
        return rows
    end

    function page.get_by_slug(value, opts)
        local only_published = opts and opts.only_published
        local sql =
            "SELECT id, title, slug, markdown_body, status FROM cms_pages WHERE slug = ?1 AND deleted_at IS NULL"
        if only_published then
            sql = sql .. " AND status = 'published'"
        end
        local rows, kind, msg = db.query(sql, { value })
        if not rows then
            return nil, kind or "storage_error", msg
        end
        if #rows == 0 then
            return nil, "not_found", "page not found"
        end
        return rows[1]
    end

    function page.upsert(payload, original_slug)
        local title, kind, msg = validate.require_non_empty(payload.title, "title")
        if not title then
            return nil, kind, msg
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

        local ok, exec_kind, exec_msg
        if original_slug and original_slug ~= "" then
            ok, exec_kind, exec_msg = db.execute(
                "UPDATE cms_pages SET title = ?1, slug = ?2, markdown_body = ?3, status = ?4, updated_at = datetime('now') WHERE slug = ?5 AND deleted_at IS NULL",
                { title, normalized_slug, body, status, original_slug }
            )
        else
            ok, exec_kind, exec_msg = db.execute(
                "INSERT INTO cms_pages (title, slug, markdown_body, status) VALUES (?1, ?2, ?3, ?4)",
                { title, normalized_slug, body, status }
            )
        end
        if not ok then
            return nil, exec_kind or "storage_error", exec_msg
        end

        return page.get_by_slug(normalized_slug)
    end

    function page.soft_delete(value)
        local ok, kind, msg = db.execute(
            "UPDATE cms_pages SET deleted_at = datetime('now'), updated_at = datetime('now') WHERE slug = ?1 AND deleted_at IS NULL",
            { value }
        )
        if not ok then
            return nil, kind or "storage_error", msg
        end
        return true
    end

    return page
end

return M
