local M = {}

local function placeholder_html(label)
    return "<tr><td>" .. label .. "</td></tr>"
end

function M.new(_deps)
    local admin = {}

    function admin.pages_table_partial(_req) return placeholder_html("pages") end
    function admin.pages_upsert_partial(_req) return placeholder_html("pages-upsert") end
    function admin.pages_delete_partial(_req) return placeholder_html("pages-delete") end

    function admin.posts_table_partial(_req) return placeholder_html("posts") end
    function admin.posts_upsert_partial(_req) return placeholder_html("posts-upsert") end
    function admin.posts_delete_partial(_req) return placeholder_html("posts-delete") end

    function admin.categories_table_partial(_req) return placeholder_html("categories") end
    function admin.categories_upsert_partial(_req) return placeholder_html("categories-upsert") end
    function admin.categories_delete_partial(_req) return placeholder_html("categories-delete") end

    return admin
end

return M
