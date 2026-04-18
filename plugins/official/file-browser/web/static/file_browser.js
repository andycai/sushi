(function () {
  function q(selector) {
    return document.querySelector(selector);
  }

  function toQuery(params) {
    const query = new URLSearchParams();
    Object.entries(params).forEach(([key, value]) => {
      query.set(key, value == null ? "" : String(value));
    });
    return query.toString();
  }

  async function fetchText(url, options) {
    const response = await fetch(url, options || {});
    const text = await response.text();
    return { ok: response.ok, status: response.status, text, headers: response.headers };
  }

  function normalizePath(path) {
    const raw = String(path || "").trim();
    if (!raw) {
      return "";
    }
    return raw.split("/").filter(Boolean).join("/");
  }

  function parentPath(path) {
    const normalized = normalizePath(path);
    if (!normalized) {
      return "";
    }
    const parts = normalized.split("/");
    parts.pop();
    return parts.join("/");
  }

  function pathChain(path) {
    const normalized = normalizePath(path);
    if (!normalized) {
      return [];
    }
    const parts = normalized.split("/");
    const chain = [];
    let current = "";
    parts.forEach((part) => {
      current = current ? `${current}/${part}` : part;
      chain.push(current);
    });
    return chain;
  }

  function fileName(path) {
    const normalized = normalizePath(path);
    if (!normalized) {
      return "";
    }
    const parts = normalized.split("/");
    return parts[parts.length - 1] || "";
  }

  window.fileBrowserPage = function fileBrowserPage(initial) {
    return {
      routePrefix: initial.routePrefix || "/app/files",
      rootId: initial.rootId || "",
      relPath: normalizePath(initial.relPath || ""),
      activePath: normalizePath(initial.relPath || ""),
      capabilities: initial.capabilities || {},
      expandedDirs: {},
      listRequestId: 0,
      contextPath: "",
      activeNodeScrollPath: "",

      init() {
        this.seedExpandedDirs();
        this.bindDelegatedEvents();
        this.refreshList();
      },

      bindDelegatedEvents() {
        document.addEventListener("click", (event) => {
          const actionEl = event.target.closest("[data-fb-action]");
          if (!actionEl) {
            if (!this.isContextMenuClick(event.target)) {
              this.closeContextMenu();
            }
            return;
          }

          const action = actionEl.getAttribute("data-fb-action");
          const path = actionEl.getAttribute("data-path") || "";

          if (action === "noop") {
            event.preventDefault();
            return;
          } else if (action === "toggle-dir") {
            event.preventDefault();
            this.toggleDirectory(path);
          } else if (action === "open-dir") {
            event.preventDefault();
            this.focusDirectory(path);
          } else if (action === "open-file") {
            event.preventDefault();
            this.openFile(path);
          } else if (action === "download") {
            event.preventDefault();
            this.download(path);
          } else if (action === "refresh-list") {
            event.preventDefault();
            this.refreshList();
          } else if (action === "ctx-create-text") {
            event.preventDefault();
            this.closeContextMenu();
            this.promptCreateText();
          } else if (action === "ctx-create-dir") {
            event.preventDefault();
            this.closeContextMenu();
            this.promptCreateDir();
          } else if (action === "ctx-rename") {
            event.preventDefault();
            this.closeContextMenu();
            this.promptRename();
          } else if (action === "ctx-delete") {
            event.preventDefault();
            this.closeContextMenu();
            this.promptDelete();
          } else if (action === "ctx-upload") {
            event.preventDefault();
            this.closeContextMenu();
            this.promptUploadToContext();
          }
        });

        document.addEventListener("contextmenu", (event) => {
          const node = event.target.closest("[data-fb-node='1'][data-kind='dir'][data-path]");
          if (!node || !this.hasContextActions()) {
            this.closeContextMenu();
            return;
          }

          event.preventDefault();
          const path = node.getAttribute("data-path") || "";
          this.openContextMenu(path, event.clientX, event.clientY);
        });

        document.addEventListener("submit", (event) => {
          const form = event.target;
          if (!(form instanceof HTMLFormElement)) {
            return;
          }

          const action = form.getAttribute("data-fb-action");
          if (action === "save-form") {
            event.preventDefault();
            this.saveText(form);
          }
        });

        document.addEventListener("change", (event) => {
          const input = event.target;
          if (!(input instanceof HTMLInputElement)) {
            return;
          }
          if (input.id !== "fb-context-upload-input") {
            return;
          }
          this.handleContextUploadInput(input);
        });

        document.addEventListener("keydown", (event) => {
          const isSave = (event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "s";
          if (event.key === "Escape") {
            this.closeContextMenu();
          }
          if (!isSave) {
            return;
          }
          event.preventDefault();
          this.saveActiveEditor();
        });

        window.addEventListener("resize", () => this.closeContextMenu());
        window.addEventListener("scroll", () => this.closeContextMenu(), true);
      },

      seedExpandedDirs() {
        pathChain(this.relPath).forEach((dirPath) => {
          this.expandedDirs[dirPath] = true;
        });
      },

      findByData(attribute, value) {
        const nodes = document.querySelectorAll(`[${attribute}]`);
        for (const node of nodes) {
          if ((node.getAttribute(attribute) || "") === value) {
            return node;
          }
        }
        return null;
      },

      findChildrenContainer(path) {
        return this.findByData("data-fb-children-for", path);
      },

      findChevron(path) {
        return this.findByData("data-fb-chevron", path);
      },

      findToggle(path) {
        const buttons = document.querySelectorAll("[data-fb-action='toggle-dir'][data-path]");
        for (const button of buttons) {
          if ((button.getAttribute("data-path") || "") === path) {
            return button;
          }
        }
        return null;
      },

      setDirectoryVisualState(path, expanded, loading) {
        const chevron = this.findChevron(path);
        if (chevron) {
          chevron.textContent = loading ? "..." : (expanded ? "▾" : "▸");
        }
        const toggle = this.findToggle(path);
        if (toggle) {
          toggle.setAttribute("aria-expanded", expanded ? "true" : "false");
          toggle.setAttribute("aria-busy", loading ? "true" : "false");
        }
      },

      clearExpandedSubtree(path) {
        Object.keys(this.expandedDirs).forEach((key) => {
          if (key === path || key.startsWith(`${path}/`)) {
            this.expandedDirs[key] = false;
          }
        });
      },

      showFlash(html) {
        const target = q("#fb-flash");
        if (target) {
          target.innerHTML = html;
        }
      },

      can(capabilityKey) {
        return this.capabilities[capabilityKey] === true;
      },

      hasContextActions() {
        return this.can("canCreateText") || this.can("canCreateDir") || this.can("canRename") || this.can("canDelete");
      },

      contextMenu() {
        return q("#fb-context-menu");
      },

      contextUploadInput() {
        return q("#fb-context-upload-input");
      },

      isContextMenuOpen() {
        const menu = this.contextMenu();
        return !!menu && !menu.classList.contains("hidden");
      },

      isContextMenuClick(target) {
        const menu = this.contextMenu();
        return !!menu && target instanceof Element && menu.contains(target);
      },

      closeContextMenu() {
        const menu = this.contextMenu();
        if (!menu) {
          return;
        }
        menu.classList.add("hidden");
      },

      openContextMenu(path, x, y) {
        const menu = this.contextMenu();
        if (!menu) {
          return;
        }

        this.contextPath = normalizePath(path);
        const label = q("#fb-context-path");
        if (label) {
          label.textContent = this.contextPath || "/";
        }

        menu.classList.remove("hidden");
        menu.style.left = `${Math.max(8, x)}px`;
        menu.style.top = `${Math.max(8, y)}px`;

        const menuRect = menu.getBoundingClientRect();
        const safeX = Math.min(Math.max(8, x), Math.max(8, window.innerWidth - menuRect.width - 8));
        const safeY = Math.min(Math.max(8, y), Math.max(8, window.innerHeight - menuRect.height - 8));
        menu.style.left = `${safeX}px`;
        menu.style.top = `${safeY}px`;
      },

      async fetchList(path) {
        const query = toQuery({ path: normalizePath(path || "") });
        const url = `${this.routePrefix}/list/${encodeURIComponent(this.rootId)}?${query}`;
        return fetchText(url);
      },

      async refreshList() {
        if (!this.rootId) {
          return;
        }
        const target = q("#fb-list");
        if (!target) {
          return;
        }

        pathChain(this.relPath).forEach((dirPath) => {
          this.expandedDirs[dirPath] = true;
        });
        const activeParent = parentPath(this.activePath);
        pathChain(activeParent).forEach((dirPath) => {
          this.expandedDirs[dirPath] = true;
        });

        const requestId = this.listRequestId + 1;
        this.listRequestId = requestId;
        const result = await this.fetchList("");
        if (requestId !== this.listRequestId) {
          return;
        }
        target.innerHTML = result.text;
        await this.restoreExpandedDirs("");
        this.syncActiveNode();
      },

      async openFile(path) {
        if (!this.rootId) {
          return;
        }
        const target = q("#fb-editor");
        if (!target) {
          return;
        }

        const normalizedPath = normalizePath(path || "");
        const query = toQuery({ path: normalizedPath });
        const url = `${this.routePrefix}/open/${encodeURIComponent(this.rootId)}?${query}`;
        const result = await fetchText(url);
        target.innerHTML = result.text;
        this.activePath = normalizedPath;
        this.relPath = parentPath(normalizedPath);
        pathChain(this.relPath).forEach((dirPath) => {
          this.expandedDirs[dirPath] = true;
        });
        await this.restoreExpandedDirs("");
        this.syncActiveNode();
      },

      async loadDirectoryChildren(path, container, shouldRestoreDescendants) {
        if (!container) {
          return false;
        }

        this.setDirectoryVisualState(path, true, true);
        const result = await this.fetchList(path);
        container.innerHTML = result.text;
        container.setAttribute("data-loaded", result.ok ? "1" : "0");
        this.setDirectoryVisualState(path, true, false);

        if (result.ok && shouldRestoreDescendants) {
          await this.restoreExpandedDirs(path);
        }
        return result.ok;
      },

      async expandDirectory(path, trackState, shouldRestoreDescendants) {
        const normalizedPath = normalizePath(path);
        if (!normalizedPath) {
          return false;
        }
        const container = this.findChildrenContainer(normalizedPath);
        if (!container) {
          return false;
        }

        if (trackState) {
          this.expandedDirs[normalizedPath] = true;
        }

        container.classList.remove("hidden");
        this.setDirectoryVisualState(normalizedPath, true, false);

        if (container.getAttribute("data-loaded") !== "1") {
          return this.loadDirectoryChildren(normalizedPath, container, shouldRestoreDescendants);
        }
        return true;
      },

      collapseDirectory(path) {
        const normalizedPath = normalizePath(path);
        if (!normalizedPath) {
          return;
        }

        const container = this.findChildrenContainer(normalizedPath);
        if (container) {
          container.classList.add("hidden");
        }

        this.clearExpandedSubtree(normalizedPath);
        this.setDirectoryVisualState(normalizedPath, false, false);
      },

      async toggleDirectory(path) {
        const normalizedPath = normalizePath(path);
        if (!normalizedPath) {
          return;
        }

        const isExpanded = this.expandedDirs[normalizedPath] === true;
        if (isExpanded) {
          this.collapseDirectory(normalizedPath);
        } else {
          this.expandedDirs[normalizedPath] = true;
          await this.expandDirectory(normalizedPath, true, true);
        }

        this.relPath = normalizedPath;
        this.activePath = normalizedPath;
        this.syncActiveNode();
      },

      async ensureDirectoryChain(path) {
        const chain = pathChain(path);
        for (const dirPath of chain) {
          this.expandedDirs[dirPath] = true;
          await this.expandDirectory(dirPath, true, false);
        }
      },

      async focusDirectory(path) {
        const normalizedPath = normalizePath(path || "");
        this.relPath = normalizedPath;
        this.activePath = normalizedPath;

        if (!normalizedPath) {
          await this.refreshList();
          return;
        }

        await this.ensureDirectoryChain(normalizedPath);
        this.syncActiveNode();
      },

      goToPath(path) {
        this.focusDirectory(path);
      },

      switchRoot() {
        const query = toQuery({ root: this.rootId || "", path: this.relPath || "" });
        window.location.href = `${this.routePrefix}?${query}`;
      },

      breadcrumbs() {
        if (!this.relPath) {
          return [];
        }
        const parts = this.relPath.split("/").filter(Boolean);
        let current = "";
        return parts.map((name) => {
          current = current ? `${current}/${name}` : name;
          return { name, path: current };
        });
      },

      goToParent() {
        if (!this.relPath) {
          return;
        }
        this.focusDirectory(parentPath(this.relPath));
      },

      async restoreExpandedDirs(scopePath) {
        const scope = normalizePath(scopePath || "");
        const expandedPaths = Object.keys(this.expandedDirs)
          .filter((path) => this.expandedDirs[path] === true)
          .filter((path) => {
            if (!scope) {
              return true;
            }
            return path !== scope && path.startsWith(`${scope}/`);
          })
          .sort((a, b) => a.split("/").length - b.split("/").length);

        for (const path of expandedPaths) {
          await this.expandDirectory(path, false, false);
        }
      },

      async postUrlEncoded(endpoint, payload) {
        return fetchText(`${this.routePrefix}${endpoint}`, {
          method: "POST",
          headers: { "Content-Type": "application/x-www-form-urlencoded" },
          body: toQuery(payload),
        });
      },

      async createTextAt(parent, name) {
        const normalizedParent = normalizePath(parent || "");
        const result = await this.postUrlEncoded("/create-text", {
          root_id: this.rootId,
          parent_path: normalizedParent,
          name: String(name || ""),
          initial_content: "",
        });
        this.showFlash(result.text);
        if (result.ok) {
          this.relPath = normalizedParent;
          pathChain(normalizedParent).forEach((dirPath) => {
            this.expandedDirs[dirPath] = true;
          });
          await this.refreshList();
        }
      },

      async createDirAt(parent, name) {
        const normalizedParent = normalizePath(parent || "");
        const result = await this.postUrlEncoded("/create-dir", {
          root_id: this.rootId,
          parent_path: normalizedParent,
          name: String(name || ""),
        });
        this.showFlash(result.text);
        if (result.ok) {
          this.relPath = normalizedParent;
          pathChain(normalizedParent).forEach((dirPath) => {
            this.expandedDirs[dirPath] = true;
          });
          await this.refreshList();
        }
      },

      async renamePath(path, newName) {
        const normalizedPath = normalizePath(path || "");
        const result = await this.postUrlEncoded("/rename", {
          root_id: this.rootId,
          path: normalizedPath,
          new_name: String(newName || ""),
        });
        this.showFlash(result.text);
        if (result.ok) {
          const currentActive = normalizePath(this.activePath || "");
          if (currentActive === normalizedPath || currentActive.startsWith(`${normalizedPath}/`)) {
            this.activePath = parentPath(normalizedPath);
          }
          await this.refreshList();
        }
      },

      async deletePath(path) {
        const normalizedPath = normalizePath(path || "");
        const result = await this.postUrlEncoded("/delete", {
          root_id: this.rootId,
          path: normalizedPath,
        });
        this.showFlash(result.text);
        if (result.ok) {
          const currentActive = normalizePath(this.activePath || "");
          if (currentActive === normalizedPath || currentActive.startsWith(`${normalizedPath}/`)) {
            this.activePath = parentPath(normalizedPath);
            this.relPath = parentPath(normalizedPath);
          }
          await this.refreshList();
        }
      },

      async uploadFileToDirectory(file, dirPath) {
        if (!file) {
          return;
        }
        const normalizedDir = normalizePath(dirPath || "");
        const bytes = await file.arrayBuffer();
        const query = toQuery({ dir: normalizedDir, name: file.name || "upload.bin" });
        const url = `${this.routePrefix}/upload/${encodeURIComponent(this.rootId)}?${query}`;

        const result = await fetchText(url, {
          method: "POST",
          headers: { "Content-Type": "application/octet-stream" },
          body: bytes,
        });

        this.showFlash(result.text);
        if (result.ok) {
          this.relPath = normalizedDir;
          this.activePath = normalizedDir;
          pathChain(normalizedDir).forEach((entryPath) => {
            this.expandedDirs[entryPath] = true;
          });
          await this.refreshList();
        }
      },

      promptCreateText() {
        if (!this.can("canCreateText")) {
          return;
        }
        const targetPath = this.contextPath || this.relPath || "";
        const name = window.prompt("New text file name", "notes.txt");
        if (!name) {
          return;
        }
        this.createTextAt(targetPath, name.trim());
      },

      promptCreateDir() {
        if (!this.can("canCreateDir")) {
          return;
        }
        const targetPath = this.contextPath || this.relPath || "";
        const name = window.prompt("New folder name", "folder");
        if (!name) {
          return;
        }
        this.createDirAt(targetPath, name.trim());
      },

      promptRename() {
        if (!this.can("canRename")) {
          return;
        }
        const targetPath = this.contextPath;
        if (!targetPath) {
          return;
        }
        const currentName = fileName(targetPath) || targetPath;
        const newName = window.prompt("New name", currentName);
        if (!newName) {
          return;
        }
        this.renamePath(targetPath, newName.trim());
      },

      promptDelete() {
        if (!this.can("canDelete")) {
          return;
        }
        const targetPath = this.contextPath;
        if (!targetPath) {
          return;
        }
        const ok = window.confirm(`Delete "${fileName(targetPath) || targetPath}" ?`);
        if (!ok) {
          return;
        }
        this.deletePath(targetPath);
      },

      promptUploadToContext() {
        if (!this.can("canUpload")) {
          return;
        }
        const input = this.contextUploadInput();
        if (!(input instanceof HTMLInputElement)) {
          return;
        }
        input.value = "";
        input.click();
      },

      handleContextUploadInput(input) {
        if (!(input instanceof HTMLInputElement)) {
          return;
        }
        if (!input.files || input.files.length === 0) {
          return;
        }
        const targetPath = this.contextPath || this.relPath || "";
        const file = input.files[0];
        input.value = "";
        this.uploadFileToDirectory(file, targetPath);
      },

      async createText(event) {
        const form = event.target;
        const formData = new FormData(form);
        await this.createTextAt(this.relPath, formData.get("name") || "");
        if (form instanceof HTMLFormElement) {
          form.reset();
        }
      },

      async createDir(event) {
        const form = event.target;
        const formData = new FormData(form);
        await this.createDirAt(this.relPath, formData.get("name") || "");
        if (form instanceof HTMLFormElement) {
          form.reset();
        }
      },

      async renameEntry(event) {
        const form = event.target;
        const formData = new FormData(form);
        await this.renamePath(formData.get("path") || "", formData.get("new_name") || "");
      },

      async deleteEntry(event) {
        const form = event.target;
        const formData = new FormData(form);
        await this.deletePath(formData.get("path") || "");
      },

      async saveText(form) {
        const formData = new FormData(form);
        const path = String(formData.get("path") || "");
        const content = String(formData.get("content") || "");
        const query = toQuery({ path });
        const url = `${this.routePrefix}/save/${encodeURIComponent(this.rootId)}?${query}`;

        const result = await fetchText(url, {
          method: "POST",
          headers: { "Content-Type": "text/plain; charset=utf-8" },
          body: content,
        });
        this.showFlash(result.text);
      },

      saveActiveEditor() {
        const form = document.querySelector("#fb-editor form[data-fb-action='save-form']");
        if (form instanceof HTMLFormElement) {
          this.saveText(form);
        }
      },

      download(path) {
        if (!this.rootId) {
          return;
        }
        const query = toQuery({ path: path || "" });
        const url = `${this.routePrefix}/download/${encodeURIComponent(this.rootId)}?${query}`;
        window.location.href = url;
      },

      syncActiveNode() {
        const nodes = document.querySelectorAll("[data-fb-node='1'][data-path]");
        let activeNode = null;
        const activePath = normalizePath(this.activePath || "");

        nodes.forEach((node) => {
          const path = node.getAttribute("data-path") || "";
          const selected = path === activePath;
          node.classList.toggle("bg-blue-100", selected);
          node.classList.toggle("border-l-2", selected);
          node.classList.toggle("border-blue-500", selected);
          if (selected) {
            activeNode = node;
          }
        });

        if (activeNode && activePath && this.activeNodeScrollPath !== activePath) {
          activeNode.scrollIntoView({ block: "nearest", inline: "nearest" });
          this.activeNodeScrollPath = activePath;
        }
      },
    };
  };
})();
