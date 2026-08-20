function app.init()
    app.capability.register({
        surface = "api",
        method = "GET",
        path = "/api/hello",
        handler = function()
            return "hello from example plugin"
        end,
        policy = "api.example.read",
    })
    app.log.info("example plugin: registered GET /api/hello")

    app.capability.register({
        surface = "cli",
        name = "hello",
        description = "Say hello from the example plugin",
        handler = function(args)
            local name = args[1] or "world"
            return "Hello, " .. name .. "!"
        end,
        policy = "cli.example.run",
    })
    app.log.info("example plugin: registered 'hello' CLI command")

    app.capability.register({
        surface = "web",
        kind = "page",
        path = "/admin/example",
        title = "Example Plugin",
        handler = function()
            return app.web.render("plugins/third_party/_example/example.html")
        end,
        policy = "admin.example.read",
    })
    app.log.info("example plugin: registered admin page /admin/example")
end
