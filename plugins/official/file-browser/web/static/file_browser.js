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

  function escapeHtml(value) {
    return String(value || "")
      .replaceAll("&", "&amp;")
      .replaceAll("<", "&lt;")
      .replaceAll(">", "&gt;")
      .replaceAll("\"", "&quot;")
      .replaceAll("'", "&#39;");
  }

  window.fileBrowserPage = function fileBrowserPage(initial) {
    return {
      routePrefix: initial.routePrefix || "/app/files",
      rootId: initial.rootId || "",
      relPath: normalizePath(initial.relPath || ""),
      activePath: normalizePath(initial.relPath || ""),
      capabilities: initial.capabilities || {},
      initialized: false,
      expandedDirs: {},
      listRequestId: 0,
      contextPath: "",
      activeNodeScrollPath: "",
      searchOpen: false,
      searchQuery: "",
      searchRequestId: 0,
      searchTimer: null,
      searchMaxResults: 200,

      init() {
        if (this.initialized) {
          return;
        }
        this.initialized = true;
        this.seedExpandedDirs();
        this.bindDelegatedEvents();
        this.setSearchToggleVisual(false);
        this.setSearchListVisibility(false);
        this.refreshList();
      },

      bindDelegatedEvents() {
        document.addEventListener("click", (event) => {
          const eventTarget = event.target instanceof Element ? event.target : event.target && event.target.parentElement;
          if (!(eventTarget instanceof Element)) {
            return;
          }

          const actionEl = eventTarget.closest("[data-fb-action]");
          if (!actionEl) {
            if (!this.isContextMenuClick(eventTarget)) {
              this.closeContextMenu();
            }
            return;
          }

          const action = actionEl.getAttribute("data-fb-action");
          const path = actionEl.getAttribute("data-path") || "";

          if (action === "noop") {
            event.preventDefault();
            return;
          } else if (action === "select-dir") {
            event.preventDefault();
            this.selectDirectory(path);
          } else if (action === "toggle-dir") {
            event.preventDefault();
            this.toggleDirectory(path, actionEl);
          } else if (action === "open-dir") {
            event.preventDefault();
            if (this.searchOpen) {
              this.closeSearchPanel();
            }
            this.focusDirectory(path);
          } else if (action === "open-file") {
            event.preventDefault();
            if (this.searchOpen) {
              this.closeSearchPanel();
            }
            this.openFile(path);
          } else if (action === "download") {
            event.preventDefault();
            this.download(path);
          } else if (action === "refresh-list") {
            event.preventDefault();
            this.refreshList();
          } else if (action === "toggle-search") {
            event.preventDefault();
            this.toggleSearchPanel();
          } else if (action === "run-search") {
            event.preventDefault();
            this.runSearchNow();
          } else if (action === "clear-search") {
            event.preventDefault();
            this.clearSearch();
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
          } else if (action === "quick-create-text") {
            event.preventDefault();
            this.triggerQuickCreateText();
          } else if (action === "quick-create-dir") {
            event.preventDefault();
            this.triggerQuickCreateDir();
          } else if (action === "quick-rename") {
            event.preventDefault();
            this.triggerQuickRename();
          } else if (action === "quick-delete") {
            event.preventDefault();
            this.triggerQuickDelete();
          } else if (action === "quick-upload") {
            event.preventDefault();
            this.triggerQuickUpload();
          }
        });

        document.addEventListener("contextmenu", (event) => {
          const eventTarget = event.target instanceof Element ? event.target : event.target && event.target.parentElement;
          if (!(eventTarget instanceof Element)) {
            return;
          }

          const node = eventTarget.closest("[data-fb-node='1'][data-kind='dir'][data-path]");
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

        document.addEventListener("input", (event) => {
          const input = event.target;
          if (!(input instanceof HTMLInputElement)) {
            return;
          }
          if (input.id !== "fb-search-input") {
            return;
          }
          this.onSearchInput(input.value);
        });

        document.addEventListener("keydown", (event) => {
          const isSave = (event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "s";
          const eventTarget = event.target;
          if (event.key === "Enter" && eventTarget instanceof HTMLInputElement && eventTarget.id === "fb-search-input") {
            event.preventDefault();
            this.runSearchNow();
            return;
          }
          if (event.key === "Escape") {
            if (this.searchOpen) {
              this.closeSearchPanel();
            }
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

      findByDataPath(attribute, path) {
        const normalizedPath = normalizePath(path || "");
        const nodes = document.querySelectorAll(`[${attribute}]`);
        for (const node of nodes) {
          const nodePath = normalizePath(node.getAttribute(attribute) || "");
          if (nodePath === normalizedPath) {
            return node;
          }
        }
        return null;
      },

      findNodeByPath(path) {
        const normalizedPath = normalizePath(path || "");
        if (!normalizedPath) {
          return null;
        }
        const nodes = document.querySelectorAll("[data-fb-node='1'][data-path]");
        for (const node of nodes) {
          if ((node.getAttribute("data-path") || "") === normalizedPath) {
            return node;
          }
        }
        return null;
      },

      findChildrenContainer(path) {
        return this.findByDataPath("data-fb-children-for", path);
      },

      resolveChildrenContainer(path, actionEl) {
        const byPath = this.findChildrenContainer(path);
        if (byPath) {
          return byPath;
        }
        if (!(actionEl instanceof Element)) {
          return null;
        }
        const node = actionEl.closest("[data-fb-node='1'][data-kind='dir'][data-path]");
        if (!node) {
          return null;
        }
        const group = node.closest(".group");
        if (!group) {
          return null;
        }
        const fallback = group.querySelector("[data-fb-children-for]");
        return fallback instanceof Element ? fallback : null;
      },

      findChevron(path) {
        return this.findByDataPath("data-fb-chevron", path);
      },

      findToggle(path) {
        const normalizedPath = normalizePath(path || "");
        const buttons = document.querySelectorAll("[data-fb-action='toggle-dir'][data-path]");
        for (const button of buttons) {
          const buttonPath = normalizePath(button.getAttribute("data-path") || "");
          if (buttonPath === normalizedPath) {
            return button;
          }
        }
        return null;
      },

      setDirectoryVisualState(path, expanded, loading) {
        const chevron = this.findChevron(path);
        if (chevron) {
          chevron.classList.toggle("rotate-90", expanded);
          chevron.classList.toggle("is-loading", loading);
        }
        const toggle = this.findToggle(path);
        if (toggle) {
          toggle.setAttribute("aria-expanded", expanded ? "true" : "false");
          toggle.setAttribute("aria-busy", loading ? "true" : "false");
        }
      },

      searchPanel() {
        return q("#fb-search-panel");
      },

      searchInput() {
        return q("#fb-search-input");
      },

      searchMeta() {
        return q("#fb-search-meta");
      },

      searchResults() {
        return q("#fb-search-results");
      },

      setSearchToggleVisual(active) {
        const toggles = document.querySelectorAll("[data-fb-search-toggle='1']");
        toggles.forEach((toggle) => {
          toggle.classList.toggle("is-active-search-toggle", active);
          toggle.setAttribute("aria-pressed", active ? "true" : "false");
        });
      },

      setSearchMeta(text) {
        const target = this.searchMeta();
        if (target) {
          target.textContent = text;
        }
      },

      showSearchHint(message) {
        const target = this.searchResults();
        if (!target) {
          return;
        }
        target.innerHTML = `<div class="px-3 py-6 text-center text-xs text-base-content">${escapeHtml(message || "")}</div>`;
      },

      setSearchListVisibility(showSearch) {
        const list = q("#fb-list");
        const results = this.searchResults();
        if (list) {
          list.classList.toggle("hidden", showSearch);
        }
        if (results) {
          results.classList.toggle("hidden", !showSearch);
        }
      },

      openSearchPanel() {
        if (this.searchOpen) {
          return;
        }
        this.searchOpen = true;
        const panel = this.searchPanel();
        if (panel) {
          panel.classList.remove("hidden");
        }
        this.setSearchToggleVisual(true);
        this.setSearchListVisibility(true);
        this.setSearchMeta("Type to search all folders in this root.");
        this.showSearchHint("Type to search all folders in this root.");
        const input = this.searchInput();
        if (input) {
          input.value = this.searchQuery || "";
          window.setTimeout(() => input.focus(), 0);
        }
      },

      closeSearchPanel() {
        this.searchOpen = false;
        this.searchQuery = "";
        this.searchRequestId += 1;
        if (this.searchTimer) {
          window.clearTimeout(this.searchTimer);
          this.searchTimer = null;
        }
        const panel = this.searchPanel();
        if (panel) {
          panel.classList.add("hidden");
        }
        const input = this.searchInput();
        if (input) {
          input.value = "";
        }
        const results = this.searchResults();
        if (results) {
          results.innerHTML = "";
        }
        this.setSearchToggleVisual(false);
        this.setSearchListVisibility(false);
      },

      toggleSearchPanel() {
        if (this.searchOpen) {
          this.closeSearchPanel();
        } else {
          this.openSearchPanel();
        }
      },

      onSearchInput(value) {
        this.searchQuery = String(value || "").trim();
        if (!this.searchOpen) {
          return;
        }
        if (this.searchTimer) {
          window.clearTimeout(this.searchTimer);
        }
        if (!this.searchQuery) {
          this.searchRequestId += 1;
          this.setSearchMeta("Type to search all folders in this root.");
          this.showSearchHint("Type to search all folders in this root.");
          this.searchTimer = null;
          return;
        }
        this.searchTimer = window.setTimeout(() => {
          this.runSearchNow();
        }, 220);
      },

      clearSearch() {
        if (!this.searchOpen) {
          return;
        }
        const input = this.searchInput();
        if (input) {
          input.value = "";
          input.focus();
        }
        this.searchQuery = "";
        this.searchRequestId += 1;
        if (this.searchTimer) {
          window.clearTimeout(this.searchTimer);
          this.searchTimer = null;
        }
        this.setSearchMeta("Type to search all folders in this root.");
        this.showSearchHint("Type to search all folders in this root.");
      },

      extractEntriesFromListHtml(html) {
        const wrapper = document.createElement("div");
        wrapper.innerHTML = html || "";
        const nodes = wrapper.querySelectorAll("[data-fb-node='1'][data-path][data-kind]");
        const entries = [];
        nodes.forEach((node) => {
          const path = normalizePath(node.getAttribute("data-path") || "");
          if (!path) {
            return;
          }
          const kind = (node.getAttribute("data-kind") || "") === "dir" ? "dir" : "file";
          entries.push({ path, kind, name: fileName(path) });
        });
        return entries;
      },

      renderSearchResults(result, query) {
        const target = this.searchResults();
        if (!target) {
          return;
        }
        const matches = result && result.matches ? result.matches : [];
        if (matches.length === 0) {
          this.setSearchMeta(`No matches for "${query}"`);
          this.showSearchHint(`No matches for "${query}"`);
          return;
        }

        const rows = matches
          .map((item) => {
            const safePath = escapeHtml(item.path);
            const safeName = escapeHtml(item.name || fileName(item.path));
            const isDir = item.kind === "dir";
            const action = isDir ? "open-dir" : "open-file";
            const badgeClass = isDir ? "badge badge-primary badge-outline badge-sm" : "badge badge-ghost badge-sm";
            const badgeText = isDir ? "DIR" : "FILE";
            return `
<li>
  <button
    type="button"
    class="fb-search-result-row"
    data-fb-action="${action}"
    data-path="${safePath}"
  >
    <span class="fb-search-result-main">
      <span class="${badgeClass}">${badgeText}</span>
      <span class="fb-search-result-name truncate">${safeName}</span>
    </span>
    <span class="fb-search-result-path fb-code-font">${safePath}</span>
  </button>
</li>`;
          })
          .join("");

        target.innerHTML = `<ul class="fb-search-result-list">${rows}</ul>`;
        const scanned = result.scanned || 0;
        const suffix = result.truncated ? " (showing first 200)" : "";
        this.setSearchMeta(`Found ${matches.length} match(es), scanned ${scanned} item(s)${suffix}.`);
      },

      async runSearchNow() {
        if (!this.searchOpen) {
          this.openSearchPanel();
        }
        if (this.searchTimer) {
          window.clearTimeout(this.searchTimer);
          this.searchTimer = null;
        }

        const query = String(this.searchQuery || "").trim();
        this.searchQuery = query;
        if (!query) {
          this.setSearchMeta("Type to search all folders in this root.");
          this.showSearchHint("Type to search all folders in this root.");
          return;
        }
        if (!this.rootId) {
          this.setSearchMeta("No root selected.");
          this.showSearchHint("No root selected.");
          return;
        }
        if (!this.can("canList")) {
          this.setSearchMeta("Search unavailable because list capability is disabled.");
          this.showSearchHint("Search unavailable because list capability is disabled.");
          return;
        }

        const requestId = this.searchRequestId + 1;
        this.searchRequestId = requestId;
        this.setSearchMeta(`Searching "${query}"...`);
        this.showSearchHint(`Searching "${query}"...`);

        const queue = [""];
        const visited = new Set([""]);
        const matches = [];
        const queryLower = query.toLowerCase();
        let scanned = 0;
        let truncated = false;

        while (queue.length > 0) {
          if (requestId !== this.searchRequestId) {
            return;
          }

          const currentPath = queue.shift();
          const result = await this.fetchList(currentPath);
          if (!result.ok) {
            continue;
          }

          const entries = this.extractEntriesFromListHtml(result.text);
          for (const entry of entries) {
            const entryPath = normalizePath(entry.path);
            if (!entryPath) {
              continue;
            }

            scanned += 1;
            const haystack = `${entry.name} ${entryPath}`.toLowerCase();
            if (haystack.includes(queryLower)) {
              matches.push(entry);
              if (matches.length >= this.searchMaxResults) {
                truncated = true;
                break;
              }
            }

            if (entry.kind === "dir" && !visited.has(entryPath)) {
              visited.add(entryPath);
              queue.push(entryPath);
            }
          }

          if (truncated) {
            break;
          }
        }

        if (requestId !== this.searchRequestId) {
          return;
        }

        matches.sort((left, right) => left.path.localeCompare(right.path));
        this.renderSearchResults({ matches, scanned, truncated }, query);
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

      showActionError(message) {
        this.showFlash(`<div class="rounded border border-rose-300 bg-rose-50 px-3 py-2 text-xs text-rose-800">${message}</div>`);
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

      resolveCurrentDirectoryPath() {
        const active = normalizePath(this.activePath || "");
        if (!active) {
          return normalizePath(this.relPath || "");
        }

        const node = this.findNodeByPath(active);
        const kind = node ? (node.getAttribute("data-kind") || "") : "";
        if (kind === "dir") {
          return active;
        }
        if (kind === "file") {
          return parentPath(active);
        }
        return normalizePath(this.relPath || "");
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

      async expandDirectory(path, trackState, shouldRestoreDescendants, actionEl) {
        const normalizedPath = normalizePath(path);
        if (!normalizedPath) {
          return false;
        }
        const container = this.resolveChildrenContainer(normalizedPath, actionEl);
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

      collapseDirectory(path, actionEl) {
        const normalizedPath = normalizePath(path);
        if (!normalizedPath) {
          return;
        }

        const container = this.resolveChildrenContainer(normalizedPath, actionEl);
        if (container) {
          container.classList.add("hidden");
        }

        this.clearExpandedSubtree(normalizedPath);
        this.setDirectoryVisualState(normalizedPath, false, false);
      },

      async toggleDirectory(path, actionEl) {
        const normalizedPath = normalizePath(path);
        if (!normalizedPath) {
          return;
        }

        const container = this.resolveChildrenContainer(normalizedPath, actionEl);
        const isVisible = container
          ? !container.classList.contains("hidden")
          : this.expandedDirs[normalizedPath] === true;

        if (isVisible) {
          this.collapseDirectory(normalizedPath, actionEl);
        } else {
          const expanded = await this.expandDirectory(normalizedPath, true, true, actionEl);
          if (!expanded) {
            await this.refreshList();
            return;
          }
        }

        this.relPath = normalizedPath;
        this.activePath = normalizedPath;
        this.syncActiveNode();
      },

      async selectDirectory(path) {
        const normalizedPath = normalizePath(path || "");
        if (!normalizedPath) {
          await this.focusDirectory("");
          return;
        }

        const isExpanded = this.expandedDirs[normalizedPath] === true;
        if (isExpanded) {
          this.collapseDirectory(normalizedPath);
          this.relPath = normalizedPath;
          this.activePath = normalizedPath;
          this.syncActiveNode();
          return;
        }

        this.expandedDirs[normalizedPath] = true;
        await this.expandDirectory(normalizedPath, true, true);
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
        if (this.searchTimer) {
          window.clearTimeout(this.searchTimer);
          this.searchTimer = null;
        }
        this.searchRequestId += 1;
        this.searchOpen = false;
        this.searchQuery = "";
        this.relPath = "";
        this.activePath = "";
        this.contextPath = "";
        this.expandedDirs = {};

        const query = new URLSearchParams();
        if (this.rootId) {
          query.set("root", this.rootId);
        }
        const suffix = query.toString();
        window.location.href = suffix ? `${this.routePrefix}?${suffix}` : this.routePrefix;
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

      triggerQuickCreateText() {
        this.contextPath = this.resolveCurrentDirectoryPath();
        this.promptCreateText();
      },

      triggerQuickCreateDir() {
        this.contextPath = this.resolveCurrentDirectoryPath();
        this.promptCreateDir();
      },

      triggerQuickRename() {
        const targetPath = normalizePath(this.activePath || "");
        if (!targetPath) {
          this.showActionError("Select a file or folder first");
          return;
        }
        this.contextPath = targetPath;
        this.promptRename();
      },

      triggerQuickDelete() {
        const targetPath = normalizePath(this.activePath || "");
        if (!targetPath) {
          this.showActionError("Select a file or folder first");
          return;
        }
        this.contextPath = targetPath;
        this.promptDelete();
      },

      triggerQuickUpload() {
        this.contextPath = this.resolveCurrentDirectoryPath();
        this.promptUploadToContext();
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
          node.classList.toggle("is-active-node", selected);
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
