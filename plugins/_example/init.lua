-- Example Sushi Plugin
-- Demonstrates route, command, and admin page registration

function sushi.init()
    -- Register a hello API route
    sushi.api.route("GET", "/api/hello", function()
        return "hello from example plugin"
    end)
    sushi.log.info("example plugin: registered GET /api/hello")

    -- Register a CLI command
    sushi.cli.command("hello", "Say hello from the example plugin")
    sushi.log.info("example plugin: registered 'hello' CLI command")

    -- Register an admin page
    sushi.admin.page("/admin/example", "Example Plugin")
    sushi.log.info("example plugin: registered admin page /admin/example")
end
