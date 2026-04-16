-- Example Sushi Plugin
-- Demonstrates route, command, and admin page registration

function sushi.init()
    -- Register a hello API route (handler takes no args, returns string)
    sushi.api.route("GET", "/api/hello", function()
        return "hello from example plugin"
    end, { policy = "api.example.read" })
    sushi.log.info("example plugin: registered GET /api/hello")

    -- Register a CLI command (handler receives args table, returns string)
    sushi.cli.command("hello", "Say hello from the example plugin", function(args)
        local name = args[1] or "world"
        return "Hello, " .. name .. "!"
    end, { policy = "cli.example.run" })
    sushi.log.info("example plugin: registered 'hello' CLI command")

    -- Register an admin page (handler returns HTML string)
    sushi.admin.page("/admin/example", "Example Plugin", function()
        return [[<!DOCTYPE html>
<html><head><title>Example Plugin</title></head>
<body><h1>Hello from Lua Plugin!</h1></body></html>]]
    end, { policy = "admin.example.read" })
    sushi.log.info("example plugin: registered admin page /admin/example")
end
