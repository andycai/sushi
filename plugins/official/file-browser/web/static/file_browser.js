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

  window.fileBrowserPage = function fileBrowserPage(initial) {
    return {
      routePrefix: initial.routePrefix || "/app/files",
      rootId: initial.rootId || "",
      relPath: normalizePath(initial.relPath || ""),
      activePath: normalizePath(initial.relPath || ""),
      expandedDirs: {},
      listRequestId: 0,

      init() {
        this.seedExpandedDirs();
        this.bindDelegatedEvents();
        this.refreshList();
      },

      bindDelegatedEvents() {
        document.addEventListener("click", (event) => {
          const actionEl = event.target.closest("[data-fb-action]");
          if (!actionEl) {
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
          }
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

        document.addEventListener("keydown", (event) => {
          const isSave = (event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "s";
          if (!isSave) {
            return;
          }
          event.preventDefault();
          this.saveActiveEditor();
        });
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

      async createText(event) {
        const form = event.target;
        const formData = new FormData(form);
        const body = toQuery({
          root_id: this.rootId,
          parent_path: this.relPath,
          name: formData.get("name") || "",
          initial_content: "",
        });

        const result = await fetchText(`${this.routePrefix}/create-text`, {
          method: "POST",
          headers: { "Content-Type": "application/x-www-form-urlencoded" },
          body,
        });
        this.showFlash(result.text);
        if (result.ok) {
          form.reset();
          this.refreshList();
        }
      },

      async createDir(event) {
        const form = event.target;
        const formData = new FormData(form);
        const body = toQuery({
          root_id: this.rootId,
          parent_path: this.relPath,
          name: formData.get("name") || "",
        });

        const result = await fetchText(`${this.routePrefix}/create-dir`, {
          method: "POST",
          headers: { "Content-Type": "application/x-www-form-urlencoded" },
          body,
        });
        this.showFlash(result.text);
        if (result.ok) {
          form.reset();
          this.refreshList();
        }
      },

      async renameEntry(event) {
        const form = event.target;
        const formData = new FormData(form);
        const body = toQuery({
          root_id: this.rootId,
          path: formData.get("path") || "",
          new_name: formData.get("new_name") || "",
        });

        const result = await fetchText(`${this.routePrefix}/rename`, {
          method: "POST",
          headers: { "Content-Type": "application/x-www-form-urlencoded" },
          body,
        });
        this.showFlash(result.text);
        if (result.ok) {
          this.refreshList();
        }
      },

      async deleteEntry(event) {
        const form = event.target;
        const formData = new FormData(form);
        const body = toQuery({
          root_id: this.rootId,
          path: formData.get("path") || "",
        });

        const result = await fetchText(`${this.routePrefix}/delete`, {
          method: "POST",
          headers: { "Content-Type": "application/x-www-form-urlencoded" },
          body,
        });
        this.showFlash(result.text);
        if (result.ok) {
          this.refreshList();
        }
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

      async uploadFile(event) {
        const form = event.target;
        const input = form.querySelector("input[type='file']");
        if (!input || !input.files || input.files.length === 0) {
          return;
        }

        const file = input.files[0];
        const bytes = await file.arrayBuffer();
        const query = toQuery({ dir: this.relPath || "", name: file.name || "upload.bin" });
        const url = `${this.routePrefix}/upload/${encodeURIComponent(this.rootId)}?${query}`;

        const result = await fetchText(url, {
          method: "POST",
          headers: { "Content-Type": "application/octet-stream" },
          body: bytes,
        });

        this.showFlash(result.text);
        if (result.ok) {
          form.reset();
          this.refreshList();
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
        nodes.forEach((node) => {
          const path = node.getAttribute("data-path") || "";
          const selected = path === normalizePath(this.activePath || "");
          node.classList.toggle("bg-blue-100", selected);
          node.classList.toggle("border-l-2", selected);
          node.classList.toggle("border-blue-500", selected);
        });
      },
    };
  };
})();
