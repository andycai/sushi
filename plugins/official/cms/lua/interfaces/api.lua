local M = {}

local function not_implemented()
    return { status = 501, body = { error = "not implemented" } }
end

function M.new(_deps)
    local api = {}

    function api.pages_list(_req) return not_implemented() end
    function api.pages_create(_req) return not_implemented() end
    function api.pages_update(_req) return not_implemented() end
    function api.pages_delete(_req) return not_implemented() end

    function api.posts_list(_req) return not_implemented() end
    function api.posts_create(_req) return not_implemented() end
    function api.posts_update(_req) return not_implemented() end
    function api.posts_delete(_req) return not_implemented() end

    function api.categories_list(_req) return not_implemented() end
    function api.categories_create(_req) return not_implemented() end
    function api.categories_update(_req) return not_implemented() end
    function api.categories_delete(_req) return not_implemented() end

    function api.public_page_detail(_req) return not_implemented() end
    function api.public_post_list(_req) return not_implemented() end
    function api.public_post_detail(_req) return not_implemented() end
    function api.public_category_detail(_req) return not_implemented() end

    return api
end

return M
