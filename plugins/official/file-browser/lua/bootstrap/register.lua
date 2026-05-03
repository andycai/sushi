local M = {}

local function register_public_route(definition)
    definition.surface = "api"
    definition.public = true
    app.capability.register(definition)
end

function M.register(app)
    register_public_route({ method = "GET", path = "/app/files", handler = app.page })
    register_public_route({ method = "GET", path = "/app/files/list/*", handler = app.list_partial })
    register_public_route({ method = "GET", path = "/app/files/open/*", handler = app.open_partial })
    register_public_route({ method = "POST", path = "/app/files/save/*", handler = app.save_text })
    register_public_route({ method = "POST", path = "/app/files/create-text", handler = app.create_text })
    register_public_route({ method = "POST", path = "/app/files/create-dir", handler = app.create_dir })
    register_public_route({ method = "POST", path = "/app/files/rename", handler = app.rename_entry })
    register_public_route({ method = "POST", path = "/app/files/delete", handler = app.delete_entry })
    register_public_route({ method = "POST", path = "/app/files/upload/*", handler = app.upload_file })
    register_public_route({ method = "GET", path = "/app/files/download/*", handler = app.download_file })
end

return M
