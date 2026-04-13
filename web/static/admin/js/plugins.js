(() => {
  function fallbackDataTable() {
    return {
      query: '',
      sortMode: 'default',
      page: 1,
      pageSize: 10,
      pageSizeOptions: [10, 20, 50],
      meta: {},
      totalRows: 0,
      filteredRows: 0,
      visibleRows: 0,
      totalPages: 1,
      emptyFiltered: false,
      apply() {},
      onAfterSwap() {},
      reset() {
        this.query = '';
      },
      setPageSize() {},
      setSortMode() {},
      prevPage() {},
      nextPage() {},
    };
  }

  window.pluginsPage = function pluginsPage() {
    const tableFactory =
      window.AdminUI && typeof window.AdminUI.createDataTable === 'function'
        ? window.AdminUI.createDataTable
        : fallbackDataTable;

    return {
      table: tableFactory({
        containerSelector: '#plugins-table-body',
        storageKey: 'admin.plugins.table.v1',
      }),
      lastUpdated: '',
      init() {},
      applySearch() {
        this.table.page = 1;
        this.table.apply('#plugins-table-body');
      },
      markLoaded() {
        this.table.onAfterSwap('#plugins-table-body');
        this.lastUpdated = window.AdminUI
          ? window.AdminUI.nowLabel()
          : new Date().toLocaleTimeString();
      },
      setPageSize() {
        this.table.setPageSize(this.table.pageSize, '#plugins-table-body');
      },
      setSortMode() {
        this.table.setSortMode(this.table.sortMode, '#plugins-table-body');
      },
      prevPage() {
        this.table.prevPage('#plugins-table-body');
      },
      nextPage() {
        this.table.nextPage('#plugins-table-body');
      },
      openWorkspace(path, title) {
        const targetPath = typeof path === 'string' ? path.trim() : '';
        if (!targetPath) {
          return;
        }

        const targetTitle =
          typeof title === 'string' && title.trim() ? title.trim() : 'Plugin Workspace';
        if (
          window.AdminWorkspace &&
          typeof window.AdminWorkspace.openTab === 'function' &&
          window.AdminWorkspace.isEnabled &&
          window.AdminWorkspace.isEnabled()
        ) {
          const opened = window.AdminWorkspace.openTab(targetPath, { title: targetTitle });
          if (opened) {
            return;
          }
        }

        window.location.href = targetPath;
      },
    };
  };

  function safeParseArray(raw) {
    if (!raw || typeof raw !== 'string') {
      return [];
    }
    try {
      const parsed = JSON.parse(raw);
      return Array.isArray(parsed) ? parsed : [];
    } catch (_) {
      return [];
    }
  }

  function canUseStorage() {
    try {
      return typeof window.localStorage !== 'undefined';
    } catch (_) {
      return false;
    }
  }

  function pluginStorageKey(pluginName, key) {
    return `admin.plugins.workspace.${pluginName}.${key}.v1`;
  }

  window.pluginWorkspacePage = function pluginWorkspacePage() {
    return {
      pluginName: '',
      pagesByPath: {},
      pinnedPaths: [],
      recentPaths: [],
      init() {
        this.pluginName = String(this.$root?.dataset?.pluginName || '').trim();
        this.collectPages();
        this.restoreState();
      },
      collectPages() {
        const rows = this.$root.querySelectorAll('[data-plugin-page-path]');
        const nextMap = {};

        rows.forEach((row) => {
          const path = String(row.dataset.pluginPagePath || '').trim();
          if (!path) {
            return;
          }
          const title = String(row.dataset.pluginPageTitle || '').trim() || path;
          nextMap[path] = title;
        });

        this.pagesByPath = nextMap;
      },
      restoreState() {
        this.pinnedPaths = this.readPathList('pinned');
        this.recentPaths = this.readPathList('recent');
      },
      readPathList(key) {
        if (!canUseStorage() || !this.pluginName) {
          return [];
        }
        const raw = window.localStorage.getItem(pluginStorageKey(this.pluginName, key));
        const parsed = safeParseArray(raw);
        return parsed.filter((path) => this.pathExists(path));
      },
      writePathList(key, paths) {
        if (!canUseStorage() || !this.pluginName) {
          return;
        }
        window.localStorage.setItem(pluginStorageKey(this.pluginName, key), JSON.stringify(paths));
      },
      pathExists(path) {
        return Object.prototype.hasOwnProperty.call(this.pagesByPath, path);
      },
      isPinned(path) {
        return this.pinnedPaths.includes(path);
      },
      togglePin(path) {
        if (!path || !this.pathExists(path)) {
          return;
        }

        if (this.isPinned(path)) {
          this.pinnedPaths = this.pinnedPaths.filter((item) => item !== path);
        } else {
          this.pinnedPaths = [path, ...this.pinnedPaths.filter((item) => item !== path)].slice(
            0,
            8,
          );
        }

        this.writePathList('pinned', this.pinnedPaths);
      },
      addRecent(path) {
        if (!path || !this.pathExists(path)) {
          return;
        }
        this.recentPaths = [path, ...this.recentPaths.filter((item) => item !== path)].slice(
          0,
          8,
        );
        this.writePathList('recent', this.recentPaths);
      },
      pageRecords(paths) {
        return paths
          .filter((path) => this.pathExists(path))
          .map((path) => ({
            path,
            title: this.pagesByPath[path] || path,
          }));
      },
      pinnedPages() {
        return this.pageRecords(this.pinnedPaths);
      },
      recentPages() {
        return this.pageRecords(this.recentPaths);
      },
      openPage(path, title) {
        const targetPath = String(path || '').trim();
        if (!targetPath) {
          return;
        }

        const pageTitle = String(title || '').trim() || this.pagesByPath[targetPath] || targetPath;
        this.addRecent(targetPath);

        if (
          window.AdminWorkspace &&
          typeof window.AdminWorkspace.openTab === 'function' &&
          window.AdminWorkspace.isEnabled &&
          window.AdminWorkspace.isEnabled()
        ) {
          const opened = window.AdminWorkspace.openTab(targetPath, { title: pageTitle });
          if (opened) {
            return;
          }
        }

        window.location.href = targetPath;
      },
    };
  };
})();
