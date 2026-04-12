(() => {
  function fallbackDataTable() {
    return {
      query: '',
      sortMode: 'default',
      page: 1,
      pageSize: 20,
      pageSizeOptions: [20, 50, 100],
      totalRows: 0,
      filteredRows: 0,
      visibleRows: 0,
      totalPages: 1,
      emptyFiltered: false,
      meta: {},
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

  window.logsPage = function logsPage() {
    const tableFactory =
      window.AdminUI && typeof window.AdminUI.createDataTable === 'function'
        ? window.AdminUI.createDataTable
        : fallbackDataTable;

    return {
      logs: [],
      error: null,
      loading: false,
      autoRefresh: false,
      autoPoller: null,
      errorNotified: false,
      levelFilter: 'ALL',
      table: tableFactory({
        containerSelector: '#logs-table-body',
        rowSelector: 'tr[data-log-row]',
        pageSizeOptions: [20, 50, 100],
        initialPageSize: 20,
        rowPredicate: (row, table) => {
          const selectedLevel = String(table.meta.level || 'ALL').toUpperCase();
          if (selectedLevel === 'ALL') {
            return true;
          }
          return String(row.dataset.logLevel || '').toUpperCase() === selectedLevel;
        },
      }),
      async init() {
        this.table.meta.level = 'ALL';
        await this.loadLogs();
        if (window.AdminUI) {
          this.autoPoller = window.AdminUI.createAutoRefresh(() => {
            this.loadLogs();
          }, 5000);
        }
        this.$watch('autoRefresh', (value) => {
          if (this.autoPoller) {
            this.autoPoller.setEnabled(value);
          }
        });
      },
      destroy() {
        if (this.autoPoller) {
          this.autoPoller.dispose();
          this.autoPoller = null;
        }
      },
      badgeTone(level) {
        if (window.AdminUI) {
          return window.AdminUI.levelTone(level);
        }
        return 'info';
      },
      applySearch() {
        this.table.page = 1;
        this.table.apply('#logs-table-body');
      },
      applyLevelFilter() {
        this.table.meta.level = this.levelFilter;
        this.table.page = 1;
        this.table.apply('#logs-table-body');
      },
      setPageSize() {
        this.table.setPageSize(this.table.pageSize, '#logs-table-body');
      },
      setSortMode() {
        this.table.setSortMode(this.table.sortMode, '#logs-table-body');
      },
      prevPage() {
        this.table.prevPage('#logs-table-body');
      },
      nextPage() {
        this.table.nextPage('#logs-table-body');
      },
      onLogRowsUpdated() {
        this.table.onAfterSwap('#logs-table-body');
      },
      async loadLogs() {
        this.loading = true;
        this.error = null;
        try {
          if (window.AdminUI) {
            const data = await window.AdminUI.fetchJson('/admin/api/logs', {}, 'load logs');
            this.logs = data.logs || [];
          } else {
            const resp = await fetch('/admin/api/logs');
            if (!resp.ok) {
              throw new Error('Failed to load logs');
            }
            const data = await resp.json();
            this.logs = data.logs || [];
          }
          this.errorNotified = false;
          this.$nextTick(() => {
            this.onLogRowsUpdated();
          });
        } catch (err) {
          this.error = 'Could not load logs. Ensure /admin/api/logs is available.';
          if (!this.errorNotified && window.AdminUI) {
            this.errorNotified = true;
            window.AdminUI.notify({
              tone: 'danger',
              title: 'Logs request failed',
              message: this.error,
            });
          }
        }
        this.loading = false;
      },
    };
  };
})();
