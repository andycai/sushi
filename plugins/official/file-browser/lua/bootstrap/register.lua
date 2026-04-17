local M = {}

function M.register(app)
    sushi.api.route("GET", "/app/files", app.page, { public = true })
    sushi.api.route("GET", "/app/files/list/*", app.list_partial, { public = true })
    sushi.api.route("GET", "/app/files/open/*", app.open_partial, { public = true })
    sushi.api.route("POST", "/app/files/save/*", app.save_text, { public = true })
    sushi.api.route("POST", "/app/files/create-text", app.create_text, { public = true })
    sushi.api.route("POST", "/app/files/create-dir", app.create_dir, { public = true })
    sushi.api.route("POST", "/app/files/rename", app.rename_entry, { public = true })
    sushi.api.route("POST", "/app/files/delete", app.delete_entry, { public = true })
    sushi.api.route("POST", "/app/files/upload/*", app.upload_file, { public = true })
    sushi.api.route("GET", "/app/files/download/*", app.download_file, { public = true })
end

return M
