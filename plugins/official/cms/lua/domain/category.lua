local M = {}

function M.new(deps)
    local domain = {}

    function domain.list() return {} end
    function domain.create(_input) return { ok = false, reason = "not implemented" } end
    function domain.update(_slug, _input) return { ok = false, reason = "not implemented" } end
    function domain.delete(_slug) return { ok = false, reason = "not implemented" } end

    domain.deps = deps
    return domain
end

return M
