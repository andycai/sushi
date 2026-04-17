local M = {}

local function render_rows(label)
    return sushi.web.render("plugins/official/cms/fragments/rows.html", {
        label = tostring(label or ""),
    })
end

function M.new(_deps)
    local admin = {}

    function admin.pages_table_partial(_req) return render_rows("pages") end
    function admin.pages_upsert_partial(_req) return render_rows("pages-upsert") end
    function admin.pages_delete_partial(_req) return render_rows("pages-delete") end

    function admin.posts_table_partial(_req) return render_rows("posts") end
    function admin.posts_upsert_partial(_req) return render_rows("posts-upsert") end
    function admin.posts_delete_partial(_req) return render_rows("posts-delete") end

    function admin.categories_table_partial(_req) return render_rows("categories") end
    function admin.categories_upsert_partial(_req) return render_rows("categories-upsert") end
    function admin.categories_delete_partial(_req) return render_rows("categories-delete") end

    return admin
end

return M
