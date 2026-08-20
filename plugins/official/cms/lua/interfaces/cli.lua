local M = {}

local function usage()
    return "Usage: sushi cms <page|post|category> <list|get|create|update|delete> [args]"
end

local function normalize_rows(rows)
    if type(rows) ~= "table" then
        return {}
    end
    return rows
end

function M.new(deps)
    local cli = {}
    local page = deps.page
    local post = deps.post
    local category = deps.category

    function cli.cms_dispatch(args)
        local resource = args[1]
        local action = args[2]
        if not resource or not action then
            return usage()
        end

        if resource == "page" and action == "list" then
            local rows = normalize_rows(page.list())
            if #rows == 0 then
                return "No pages found."
            end
            local lines = {}
            for i = 1, #rows do
                lines[#lines + 1] = rows[i].slug .. " [" .. rows[i].status .. "]"
            end
            return table.concat(lines, "\n")
        end

        if resource == "category" and action == "list" then
            local rows = normalize_rows(category.list())
            if #rows == 0 then
                return "No categories found."
            end
            local lines = {}
            for i = 1, #rows do
                lines[#lines + 1] = rows[i].slug
            end
            return table.concat(lines, "\n")
        end

        if resource == "post" and action == "list" then
            local rows = normalize_rows(post.list({ only_published = false }))
            if #rows == 0 then
                return "No posts found."
            end
            local lines = {}
            for i = 1, #rows do
                lines[#lines + 1] = rows[i].slug .. " [" .. rows[i].status .. "]"
            end
            return table.concat(lines, "\n")
        end

        return usage()
    end

    return cli
end

return M
