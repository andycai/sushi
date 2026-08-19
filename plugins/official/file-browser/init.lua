-- File Browser official plugin entry.
-- Composition root only; implementation lives under lua/* modules.

local browser_domain = require("domain.browser")
local web_factory = require("interfaces.web")
local register = require("bootstrap.register")

function app.init()
    local browser = browser_domain.new()
    local web = web_factory.new({ browser = browser })
    register.register(web)
    app.log.info("file-browser plugin: registered public web routes")
end
