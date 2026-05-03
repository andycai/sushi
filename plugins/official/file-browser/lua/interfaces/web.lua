local form_utils = require("utils.form")
local path_utils = require("utils.path")

local M = {}

local LIST_PREFIX = "/app/files/list/"
local OPEN_PREFIX = "/app/files/open/"
local SAVE_PREFIX = "/app/files/save/"
local UPLOAD_PREFIX = "/app/files/upload/"
local DOWNLOAD_PREFIX = "/app/files/download/"

local function status_for_error(kind)
    if kind == "permission_denied" then
        return 403
    end
    if kind == "not_found" then
        return 404
    end
    if kind == "invalid_path" or kind == "not_text_file" or kind == "conflict" then
        return 400
    end
    return 500
end

local function friendly_error(kind, message)
    local fallback = tostring(message or kind or "operation failed")
    if kind == "permission_denied" then
        return "Operation denied by root capability settings"
    end
    if kind == "forbidden_hidden" then
        return "Hidden paths are not allowed"
    end
    if kind == "forbidden_symlink" then
        return "Symlink paths are not allowed"
    end
    if kind == "not_text_file" then
        return "This file is not in the configured text extension whitelist"
    end
    if kind == "not_empty_dir" then
        return "Directory is not empty"
    end
    if kind == "conflict" then
        return "Target already exists"
    end
    if kind == "root_not_found" then
        return "No file browser roots are configured"
    end
    return fallback
end

local function ensure_rel_path(value)
    return path_utils.normalize_rel_path(value)
end

local function render_flash(tone, message)
    return app.web.render("plugins/official/file-browser/fragments/flash.html", {
        tone = tostring(tone or "info"),
        message = tostring(message or ""),
    })
end

local function root_id_or_unknown(root, fallback_root_id)
    if root and root.id and root.id ~= "" then
        return root.id
    end
    if fallback_root_id and fallback_root_id ~= "" then
        return fallback_root_id
    end
    return "unknown"
end

local function list_context(browser, root_id, rel_path)
    local root, resolved_root_id = browser.root(root_id)
    local safe_rel_path = ensure_rel_path(rel_path)
    local entries = {}
    local list_error = nil

    if root and root.capabilities and root.capabilities.can_list then
        local rows, kind, message = browser.list(resolved_root_id, safe_rel_path)
        if rows then
            entries = rows
        else
            list_error = friendly_error(kind, message)
        end
    else
        local root_name = root_id_or_unknown(root, resolved_root_id)
        list_error = string.format(
            "List operation is disabled for root '%s'. Check [file_browser.roots.capabilities].can_list in plugin.toml.",
            root_name
        )
    end

    return {
        root = root,
        root_id = resolved_root_id,
        rel_path = safe_rel_path,
        parent_path = path_utils.parent_path(safe_rel_path),
        entries = entries,
        list_error = list_error,
    }
end

local function parse_root_from_wildcard(args, prefix)
    local path = (args and args[1]) or ""
    return path_utils.extract_root_id(path, prefix)
end

function M.new(deps)
    local browser = deps.browser
    local web = {}

    function web.page(args)
        local dispatch = (args and args.dispatch_path) or ""
        local _, query = path_utils.split_path_and_query(dispatch)
        local query_map = path_utils.parse_query(query)

        local requested_root = query_map.root or ""
        local rel_path = ensure_rel_path(query_map.path or "")

        local roots = browser.roots()
        local default_root_id = browser.default_root_id()
        local resolved_root_id = browser.resolve_root_id(requested_root)

        local list = list_context(browser, resolved_root_id, rel_path)
        local root = list.root

        return app.web.render("plugins/official/file-browser/file_browser.html", {
            route_prefix = browser.route_prefix(),
            asset_version = tostring(os.time()),
            roots = roots,
            has_roots = #roots > 0,
            root = root,
            root_id = resolved_root_id,
            rel_path = rel_path,
            parent_path = list.parent_path,
            entries = list.entries,
            list_error = list.list_error,
            default_root_id = default_root_id,
        })
    end

    function web.list_partial(args)
        local root_id = parse_root_from_wildcard(args, LIST_PREFIX)
        local rel_path = ensure_rel_path(path_utils.read_query_value(args, "path") or "")

        local list = list_context(browser, root_id, rel_path)

        return app.web.render("plugins/official/file-browser/fragments/list.html", {
            root = list.root,
            root_id = list.root_id,
            rel_path = list.rel_path,
            parent_path = list.parent_path,
            entries = list.entries,
            list_error = list.list_error,
        })
    end

    function web.open_partial(args)
        local root_id = parse_root_from_wildcard(args, OPEN_PREFIX)
        local rel_path = ensure_rel_path(path_utils.read_query_value(args, "path") or "")
        local root, resolved_root_id = browser.root(root_id)

        local content, kind, message = browser.read_text(resolved_root_id, rel_path)

        local is_text = content ~= nil
        local open_error = nil
        local value = ""

        if is_text then
            value = content
        else
            open_error = friendly_error(kind, message)
            if kind == "not_text_file" then
                open_error = "This file is not text. Use download instead."
            end
        end

        return app.web.render("plugins/official/file-browser/fragments/editor.html", {
            root = root,
            root_id = resolved_root_id,
            rel_path = rel_path,
            file_name = path_utils.file_name(rel_path),
            content = value,
            is_text = is_text,
            can_edit = root and root.capabilities and root.capabilities.can_edit_text or false,
            can_download = root and root.capabilities and root.capabilities.can_download or false,
            open_error = open_error,
        })
    end

    function web.save_text(args)
        local root_id = parse_root_from_wildcard(args, SAVE_PREFIX)
        local rel_path = ensure_rel_path(path_utils.read_query_value(args, "path") or "")
        local body = (args and args[2]) or ""

        local ok, kind, message = browser.write_text(root_id, rel_path, body)
        if not ok then
            return render_flash("error", friendly_error(kind, message))
        end
        return render_flash("success", "Saved " .. path_utils.file_name(rel_path))
    end

    function web.create_text(args)
        local body = (args and args[2]) or ""
        local parsed = form_utils.parse_urlencoded(body)
        local root_id = tostring(parsed.root_id or "")
        local parent_path = ensure_rel_path(parsed.parent_path or "")
        local name = path_utils.clean_name(parsed.name or "")
        local initial_content = tostring(parsed.initial_content or "")

        if name == "" then
            return render_flash("error", "File name is required")
        end
        if not name:lower():match("%.txt$") then
            name = name .. ".txt"
        end

        local rel_path = path_utils.join_rel(parent_path, name)
        local ok, kind, message = browser.create_text(root_id, rel_path, initial_content)
        if not ok then
            return render_flash("error", friendly_error(kind, message))
        end
        return render_flash("success", "Created " .. name)
    end

    function web.create_dir(args)
        local body = (args and args[2]) or ""
        local parsed = form_utils.parse_urlencoded(body)
        local root_id = tostring(parsed.root_id or "")
        local parent_path = ensure_rel_path(parsed.parent_path or "")
        local name = path_utils.clean_name(parsed.name or "")

        if name == "" then
            return render_flash("error", "Directory name is required")
        end

        local rel_path = path_utils.join_rel(parent_path, name)
        local ok, kind, message = browser.create_dir(root_id, rel_path)
        if not ok then
            return render_flash("error", friendly_error(kind, message))
        end
        return render_flash("success", "Created directory " .. name)
    end

    function web.rename_entry(args)
        local body = (args and args[2]) or ""
        local parsed = form_utils.parse_urlencoded(body)
        local root_id = tostring(parsed.root_id or "")
        local from_path = ensure_rel_path(parsed.path or "")
        local new_name = path_utils.clean_name(parsed.new_name or "")

        if from_path == "" then
            return render_flash("error", "Missing source path")
        end
        if new_name == "" then
            return render_flash("error", "New name is required")
        end

        local parent_path = path_utils.parent_path(from_path)
        local to_path = path_utils.join_rel(parent_path, new_name)
        local ok, kind, message = browser.rename(root_id, from_path, to_path)
        if not ok then
            return render_flash("error", friendly_error(kind, message))
        end
        return render_flash("success", "Renamed to " .. new_name)
    end

    function web.delete_entry(args)
        local body = (args and args[2]) or ""
        local parsed = form_utils.parse_urlencoded(body)
        local root_id = tostring(parsed.root_id or "")
        local rel_path = ensure_rel_path(parsed.path or "")

        if rel_path == "" then
            return render_flash("error", "Missing target path")
        end

        local ok, kind, message = browser.delete(root_id, rel_path)
        if not ok then
            return render_flash("error", friendly_error(kind, message))
        end
        return render_flash("success", "Deleted " .. path_utils.file_name(rel_path))
    end

    function web.upload_file(args)
        local root_id = parse_root_from_wildcard(args, UPLOAD_PREFIX)
        local dir_path = ensure_rel_path(path_utils.read_query_value(args, "dir") or "")
        local file_name = path_utils.clean_name(path_utils.read_query_value(args, "name") or "")
        local body = (args and args[2]) or ""

        if file_name == "" then
            return render_flash("error", "Upload file name is required")
        end

        local rel_path = path_utils.join_rel(dir_path, file_name)
        local ok, kind, message = browser.upload(root_id, rel_path, body)
        if not ok then
            return render_flash("error", friendly_error(kind, message))
        end
        return render_flash("success", "Uploaded " .. file_name)
    end

    function web.download_file(args)
        local root_id = parse_root_from_wildcard(args, DOWNLOAD_PREFIX)
        local rel_path = ensure_rel_path(path_utils.read_query_value(args, "path") or "")

        local payload, kind, message = browser.read_download(root_id, rel_path)
        if not payload then
            return app.web.json(status_for_error(kind), {
                error = friendly_error(kind, message),
            })
        end

        return app.web.download(payload.file_name, "application/octet-stream", payload.content)
    end

    return web
end

return M
