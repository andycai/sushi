(() => {
  function fallbackDataTable() {
    return {
      query: '',
      totalRows: 0,
      visibleRows: 0,
      emptyFiltered: false,
      apply() {},
      onAfterSwap() {},
      reset() {
        this.query = '';
      },
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
      }),
      lastUpdated: '',
      init() {
        this.applySearch();
      },
      applySearch() {
        this.table.apply('#plugins-table-body');
      },
      markLoaded() {
        this.table.onAfterSwap('#plugins-table-body');
        this.lastUpdated = window.AdminUI
          ? window.AdminUI.nowLabel()
          : new Date().toLocaleTimeString();
      },
    };
  };
})();
