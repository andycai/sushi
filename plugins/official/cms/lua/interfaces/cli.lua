local M = {}

function M.new(_deps)
    local cli = {}

    function cli.cms_dispatch(_args)
        sushi.log.info("cms command scaffold is not implemented")
    end

    return cli
end

return M
