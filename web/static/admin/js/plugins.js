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
    };
  };
})();
