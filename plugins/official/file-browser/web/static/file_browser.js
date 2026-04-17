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

  window.fileBrowserPage = function fileBrowserPage(initial) {
    return {
      routePrefix: initial.routePrefix || "/app/files",
      rootId: initial.rootId || "",
      relPath: initial.relPath || "",

      init() {
        this.bindDelegatedEvents();
      },

      bindDelegatedEvents() {
        document.addEventListener("click", (event) => {
          const actionEl = event.target.closest("[data-fb-action]");
          if (!actionEl) {
            return;
          }

          const action = actionEl.getAttribute("data-fb-action");
          const path = actionEl.getAttribute("data-path") || "";

          if (action === "open-dir") {
            event.preventDefault();
            this.goToPath(path);
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
      },

      showFlash(html) {
        const target = q("#fb-flash");
        if (target) {
          target.innerHTML = html;
        }
      },

      async refreshList() {
        if (!this.rootId) {
          return;
        }
        const target = q("#fb-list");
        if (!target) {
          return;
        }

        const query = toQuery({ path: this.relPath || "" });
        const url = `${this.routePrefix}/list/${encodeURIComponent(this.rootId)}?${query}`;
        const result = await fetchText(url);
        target.innerHTML = result.text;
      },

      async openFile(path) {
        if (!this.rootId) {
          return;
        }
        const target = q("#fb-editor");
        if (!target) {
          return;
        }

        const query = toQuery({ path: path || "" });
        const url = `${this.routePrefix}/open/${encodeURIComponent(this.rootId)}?${query}`;
        const result = await fetchText(url);
        target.innerHTML = result.text;
      },

      goToPath(path) {
        this.relPath = path || "";
        this.refreshList();
      },

      switchRoot() {
        const query = toQuery({ root: this.rootId || "", path: this.relPath || "" });
        window.location.href = `${this.routePrefix}?${query}`;
      },

      goToParent() {
        if (!this.relPath) {
          return;
        }
        const parts = this.relPath.split("/").filter(Boolean);
        parts.pop();
        this.relPath = parts.join("/");
        this.refreshList();
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
    };
  };
})();
