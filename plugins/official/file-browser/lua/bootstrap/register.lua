local M = {}

local function register_public_route(definition)
    definition.surface = "api"
    definition.public = true
    app.capability.register(definition)
end

function M.register(web)
    local config = app.config.get("file_browser") or {}
    local prefix = tostring(config.route_prefix or "/app/files")
    local function route(suffix)
        return prefix .. suffix
    end

    register_public_route({ method = "GET", path = route(""), handler = web.page })
    register_public_route({ method = "GET", path = route("/list/*"), handler = web.list_partial })
    register_public_route({ method = "GET", path = route("/open/*"), handler = web.open_partial })
    register_public_route({ method = "POST", path = route("/save/*"), handler = web.save_text })
    register_public_route({ method = "POST", path = route("/create-text"), handler = web.create_text })
    register_public_route({ method = "POST", path = route("/create-dir"), handler = web.create_dir })
    register_public_route({ method = "POST", path = route("/rename"), handler = web.rename_entry })
    register_public_route({ method = "POST", path = route("/delete"), handler = web.delete_entry })
    register_public_route({ method = "POST", path = route("/upload/*"), handler = web.upload_file })
    register_public_route({ method = "GET", path = route("/download/*"), handler = web.download_file })
end

return M
