-- KV Store official plugin entry.
-- Composition root only; implementation lives under lua/* modules.

local json_utils = require("utils.json")
local form_utils = require("utils.form")
local html_utils = require("utils.html")

local db = require("infra.db")
local store_factory = require("domain.store")

local api_factory = require("interfaces.api")
local admin_factory = require("interfaces.admin")
local cli_factory = require("interfaces.cli")

local bootstrap = require("bootstrap.register")

function app.init()
    local store = store_factory.new({ db = db })

    local api = api_factory.new({
        store = store,
        json = json_utils,
    })

    local admin = admin_factory.new({
        store = store,
        form = form_utils,
        html = html_utils,
    })

    local cli = cli_factory.new({
        store = store,
    })

    bootstrap.register({
        api = api,
        admin = admin,
        cli = cli,
    })

    app.log.info("kv-store plugin: registered API routes, admin page, and CLI commands")
end
