-- CMS official plugin entry.
-- Composition root only; behavior lives under lua/* modules.

local db = require("infra.db")
local slug = require("utils.slug")
local validate = require("utils.validate")
local markdown = require("utils.markdown")

local page_domain = require("domain.page")
local post_domain = require("domain.post")
local category_domain = require("domain.category")

local api_factory = require("interfaces.api")
local admin_factory = require("interfaces.admin")
local cli_factory = require("interfaces.cli")
local bootstrap = require("bootstrap.register")

function app.init()
    local deps = {
        db = db,
        slug = slug,
        validate = validate,
        markdown = markdown,
    }

    local page = page_domain.new(deps)
    local post = post_domain.new(deps)
    local category = category_domain.new(deps)

    local api = api_factory.new({
        page = page,
        post = post,
        category = category,
        markdown = markdown,
    })
    local admin = admin_factory.new({
        page = page,
        post = post,
        category = category,
        markdown = markdown,
    })
    local cli = cli_factory.new({
        page = page,
        post = post,
        category = category,
    })

    bootstrap.register({
        api = api,
        admin = admin,
        cli = cli,
    })

    app.log.info("cms plugin: registered API routes, admin page, and CLI commands")
end
