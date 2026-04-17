local M = {}

local function parse_fs_error(err)
    local message = tostring(err or "runtime error")
    local known_codes = {
        "invalid_path",
        "root_not_found",
        "permission_denied",
        "forbidden_hidden",
        "forbidden_symlink",
        "not_text_file",
        "not_found",
        "conflict",
        "not_empty_dir",
        "io_error",
    }

    for _, code in ipairs(known_codes) do
        if message:find(code, 1, true) then
            return code, message
        end
    end

    return "io_error", message
end

local function capability_enabled(root, capability_key)
    if not root or not root.capabilities then
        return false
    end
    return root.capabilities[capability_key] == true
end

local function operation_name(capability_key)
    local names = {
        can_list = "list",
        can_view_text = "view text",
        can_edit_text = "edit text",
        can_create_text = "create text",
        can_create_dir = "create directory",
        can_rename = "rename",
        can_delete = "delete",
        can_upload = "upload",
        can_download = "download",
    }
    return names[capability_key] or capability_key
end

function M.new()
    local browser = {}

    local roots = sushi.fs.roots() or {}
    local root_by_id = {}
    for _, root in ipairs(roots) do
        if root and root.id then
            root_by_id[root.id] = root
        end
    end

    local function resolve_root_id(root_id)
        if root_id and root_id ~= "" and root_by_id[root_id] then
            return root_id
        end
        if #roots > 0 then
            return roots[1].id
        end
        return ""
    end

    local function call_fs(root_id, rel_path, capability_key, fn)
        local resolved_root = resolve_root_id(root_id)
        if resolved_root == "" then
            return nil, "root_not_found", "No file browser roots are configured"
        end

        local root = root_by_id[resolved_root]
        if not capability_enabled(root, capability_key) then
            return nil, "permission_denied", "Capability denied: " .. operation_name(capability_key)
        end

        local ok, result = pcall(fn, resolved_root, rel_path)
        if not ok then
            local code, message = parse_fs_error(result)
            return nil, code, message
        end
        return result, nil, nil
    end

    function browser.roots()
        return roots
    end

    function browser.default_root_id()
        return resolve_root_id("")
    end

    function browser.route_prefix()
        return tostring(sushi.fs.route_prefix or "/app/files")
    end

    function browser.resolve_root_id(root_id)
        return resolve_root_id(root_id)
    end

    function browser.root(root_id)
        local resolved = resolve_root_id(root_id)
        return root_by_id[resolved], resolved
    end

    function browser.list(root_id, rel_path)
        return call_fs(root_id, rel_path or "", "can_list", sushi.fs.list)
    end

    function browser.read_text(root_id, rel_path)
        return call_fs(root_id, rel_path or "", "can_view_text", sushi.fs.read_text)
    end

    function browser.write_text(root_id, rel_path, content)
        local resolved_root = resolve_root_id(root_id)
        local root = root_by_id[resolved_root]
        if not capability_enabled(root, "can_edit_text") then
            return nil, "permission_denied", "Capability denied: " .. operation_name("can_edit_text")
        end

        local ok, result = pcall(sushi.fs.write_text, resolved_root, rel_path or "", content or "")
        if not ok then
            local code, message = parse_fs_error(result)
            return nil, code, message
        end
        return true, nil, nil
    end

    function browser.create_text(root_id, rel_path, initial_content)
        local resolved_root = resolve_root_id(root_id)
        local root = root_by_id[resolved_root]
        if not capability_enabled(root, "can_create_text") then
            return nil, "permission_denied", "Capability denied: " .. operation_name("can_create_text")
        end

        local ok, result = pcall(sushi.fs.create_text, resolved_root, rel_path or "", initial_content)
        if not ok then
            local code, message = parse_fs_error(result)
            return nil, code, message
        end
        return true, nil, nil
    end

    function browser.create_dir(root_id, rel_path)
        local resolved_root = resolve_root_id(root_id)
        local root = root_by_id[resolved_root]
        if not capability_enabled(root, "can_create_dir") then
            return nil, "permission_denied", "Capability denied: " .. operation_name("can_create_dir")
        end

        local ok, result = pcall(sushi.fs.mkdir, resolved_root, rel_path or "")
        if not ok then
            local code, message = parse_fs_error(result)
            return nil, code, message
        end
        return true, nil, nil
    end

    function browser.rename(root_id, from_rel_path, to_rel_path)
        local resolved_root = resolve_root_id(root_id)
        local root = root_by_id[resolved_root]
        if not capability_enabled(root, "can_rename") then
            return nil, "permission_denied", "Capability denied: " .. operation_name("can_rename")
        end

        local ok, result = pcall(sushi.fs.rename, resolved_root, from_rel_path or "", to_rel_path or "")
        if not ok then
            local code, message = parse_fs_error(result)
            return nil, code, message
        end
        return true, nil, nil
    end

    function browser.delete(root_id, rel_path)
        local resolved_root = resolve_root_id(root_id)
        local root = root_by_id[resolved_root]
        if not capability_enabled(root, "can_delete") then
            return nil, "permission_denied", "Capability denied: " .. operation_name("can_delete")
        end

        local ok, result = pcall(sushi.fs.delete, resolved_root, rel_path or "")
        if not ok then
            local code, message = parse_fs_error(result)
            return nil, code, message
        end
        return true, nil, nil
    end

    function browser.upload(root_id, rel_path, content)
        local resolved_root = resolve_root_id(root_id)
        local root = root_by_id[resolved_root]
        if not capability_enabled(root, "can_upload") then
            return nil, "permission_denied", "Capability denied: " .. operation_name("can_upload")
        end

        local ok, result = pcall(sushi.fs.write_upload, resolved_root, rel_path or "", content or "")
        if not ok then
            local code, message = parse_fs_error(result)
            return nil, code, message
        end
        return true, nil, nil
    end

    function browser.read_download(root_id, rel_path)
        local resolved_root = resolve_root_id(root_id)
        local root = root_by_id[resolved_root]
        if not capability_enabled(root, "can_download") then
            return nil, "permission_denied", "Capability denied: " .. operation_name("can_download")
        end

        local ok, result = pcall(sushi.fs.read_download, resolved_root, rel_path or "")
        if not ok then
            local code, message = parse_fs_error(result)
            return nil, code, message
        end
        return result, nil, nil
    end

    return browser
end

return M
