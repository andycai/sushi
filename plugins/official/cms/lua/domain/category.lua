local M = {}

function M.new(deps)
    local db = deps.db
    local validate = deps.validate
    local slug = deps.slug
    local category = {}

    function category.list()
        local rows, kind, msg = db.query(
            "SELECT id, name, slug, description, created_at, updated_at FROM cms_categories WHERE deleted_at IS NULL ORDER BY updated_at DESC",
            {}
        )
        if not rows then
            return nil, kind or "storage_error", msg
        end
        return rows
    end

    function category.get_by_slug(value)
        local rows, kind, msg = db.query(
            "SELECT id, name, slug, description, created_at, updated_at FROM cms_categories WHERE slug = ?1 AND deleted_at IS NULL",
            { value }
        )
        if not rows then
            return nil, kind or "storage_error", msg
        end
        if #rows == 0 then
            return nil, "not_found", "category not found"
        end
        return rows[1]
    end

    function category.upsert(payload, original_slug)
        local name, name_kind, name_msg = validate.require_non_empty(payload.name, "name")
        if not name then
            return nil, name_kind, name_msg
        end
        local slug_input, slug_kind, slug_msg = validate.require_non_empty(payload.slug, "slug")
        if not slug_input then
            return nil, slug_kind, slug_msg
        end
        local normalized_slug = slug.normalize(slug_input)
        if normalized_slug == "" then
            return nil, "invalid_slug", "slug cannot be empty"
        end
        local description = payload.description

        local ok, kind, msg
        if original_slug and original_slug ~= "" then
            ok, kind, msg = db.execute(
                "UPDATE cms_categories SET name = ?1, slug = ?2, description = ?3, updated_at = datetime('now') WHERE slug = ?4 AND deleted_at IS NULL",
                { name, normalized_slug, description, original_slug }
            )
        else
            ok, kind, msg = db.execute(
                "INSERT INTO cms_categories (name, slug, description) VALUES (?1, ?2, ?3)",
                { name, normalized_slug, description }
            )
        end
        if not ok then
            return nil, kind or "storage_error", msg
        end

        return category.get_by_slug(normalized_slug)
    end

    function category.soft_delete(slug_value)
        local rows, kind, msg = db.query(
            "SELECT id FROM cms_categories WHERE slug = ?1 AND deleted_at IS NULL",
            { slug_value }
        )
        if not rows then
            return nil, kind or "storage_error", msg
        end
        if #rows == 0 then
            return nil, "not_found", "category not found"
        end

        local category_id = rows[1].id
        local refs, ref_kind, ref_msg = db.query(
            "SELECT id FROM cms_posts WHERE category_id = ?1 AND deleted_at IS NULL LIMIT 1",
            { category_id }
        )
        if not refs then
            return nil, ref_kind or "storage_error", ref_msg
        end
        if #refs > 0 then
            return nil, "conflict_has_posts", "category still has posts"
        end

        local ok, exec_kind, exec_msg = db.execute(
            "UPDATE cms_categories SET deleted_at = datetime('now'), updated_at = datetime('now') WHERE id = ?1",
            { category_id }
        )
        if not ok then
            return nil, exec_kind or "storage_error", exec_msg
        end
        return true
    end

    return category
end

return M
