(() => {
  function fallbackModal(factory) {
    const createPayload =
      typeof factory === 'function' ? factory : () => ({});

    return {
      open: false,
      busy: false,
      payload: createPayload(),
      show(payload = {}) {
        this.open = true;
        this.busy = false;
        this.payload = {
          ...createPayload(),
          ...payload,
        };
      },
      hide() {
        this.open = false;
        this.busy = false;
        this.payload = createPayload();
      },
    };
  }

  function fallbackForm(factory) {
    const createValues =
      typeof factory === 'function' ? factory : () => ({});

    return {
      busy: false,
      values: createValues(),
      reset(values = {}) {
        this.busy = false;
        this.values = {
          ...createValues(),
          ...values,
        };
      },
    };
  }

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

  window.kvPage = function kvPage() {
    const makeEntryForm = () => ({
      key: '',
      value: '',
      originalKey: '',
      mode: 'create',
    });
    const makeDeletePayload = () => ({
      key: '',
    });

    const modalFactory =
      window.AdminUI && typeof window.AdminUI.createModal === 'function'
        ? window.AdminUI.createModal
        : fallbackModal;
    const drawerFactory =
      window.AdminUI && typeof window.AdminUI.createDrawer === 'function'
        ? window.AdminUI.createDrawer
        : fallbackModal;
    const formFactory =
      window.AdminUI && typeof window.AdminUI.createForm === 'function'
        ? window.AdminUI.createForm
        : fallbackForm;
    const dataTableFactory =
      window.AdminUI && typeof window.AdminUI.createDataTable === 'function'
        ? window.AdminUI.createDataTable
        : fallbackDataTable;

    return {
      table: dataTableFactory({
        containerSelector: '#kv-table-body',
        storageKey: 'admin.kv.table.v1',
      }),
      editorDrawer: drawerFactory(() => ({})),
      entryForm: formFactory(makeEntryForm),
      confirmModal: modalFactory(makeDeletePayload),
      init() {},
      get mode() {
        return this.entryForm.values.mode || 'create';
      },
      openCreate() {
        this.confirmModal.hide();
        this.entryForm.reset({ mode: 'create' });
        this.editorDrawer.show();
      },
      openEdit(key, value) {
        this.confirmModal.hide();
        this.entryForm.reset({
          key,
          value,
          originalKey: key,
          mode: 'edit',
        });
        this.editorDrawer.show();
      },
      closeModal() {
        this.editorDrawer.hide();
        this.entryForm.busy = false;
      },
      openDeleteConfirm(key) {
        this.editorDrawer.hide();
        this.confirmModal.show({ key: key || '' });
      },
      closeDeleteConfirm() {
        this.confirmModal.hide();
      },
      applySearch() {
        this.table.page = 1;
        this.table.apply('#kv-table-body');
      },
      onKvTableSwap() {
        this.table.onAfterSwap('#kv-table-body');
      },
      setPageSize() {
        this.table.setPageSize(this.table.pageSize, '#kv-table-body');
      },
      setSortMode() {
        this.table.setSortMode(this.table.sortMode, '#kv-table-body');
      },
      prevPage() {
        this.table.prevPage('#kv-table-body');
      },
      nextPage() {
        this.table.nextPage('#kv-table-body');
      },
      notifyFeedback(selector, fallbackLevel) {
        if (window.AdminUI && typeof window.AdminUI.consumeFeedback === 'function') {
          const consumed = window.AdminUI.consumeFeedback(selector, fallbackLevel);
          if (!consumed && fallbackLevel === 'error') {
            window.AdminUI.notify({
              tone: 'danger',
              title: 'Request failed',
              message: 'Unable to complete the KV operation.',
            });
          }
        }
      },
      isErrorFeedback(selector) {
        if (window.AdminUI && typeof window.AdminUI.isErrorFeedback === 'function') {
          return window.AdminUI.isErrorFeedback(selector, 'error');
        }

        const container = document.querySelector(selector);
        if (!container) {
          return false;
        }

        const flash = container.querySelector('[data-ui-flash]');
        if (!flash) {
          return false;
        }

        const level = String(flash.dataset.level || '').toLowerCase();
        return level === 'error' || level === 'danger';
      },
      isSuccessfulKvRequest(event, feedbackSelector) {
        if (!event?.detail?.successful) {
          return false;
        }
        return !this.isErrorFeedback(feedbackSelector);
      },
      refreshTable() {
        if (window.AdminUI && typeof window.AdminUI.refreshPartial === 'function') {
          window.AdminUI.refreshPartial({
            url: '/admin/partials/kv/table',
            target: '#kv-table-body',
            onAfterSwap: () => this.onKvTableSwap(),
            errorMessage: 'Unable to refresh the KV list.',
          });
          return;
        }

        fetch('/admin/partials/kv/table')
          .then((response) => {
            if (!response.ok) {
              throw new Error(`Failed to refresh kv entries (${response.status})`);
            }
            return response.text();
          })
          .then((html) => {
            const target = document.querySelector('#kv-table-body');
            if (!target) {
              return;
            }
            target.innerHTML = html;
            this.onKvTableSwap();
          })
          .catch(() => {
            if (window.AdminUI) {
              window.AdminUI.notify({
                tone: 'danger',
                title: 'Refresh failed',
                message: 'Unable to refresh the KV list.',
              });
            }
          });
      },
      onUpsertBeforeRequest() {
        this.entryForm.busy = true;
      },
      onUpsertAfterRequest(event) {
        this.entryForm.busy = false;
        const successful = this.isSuccessfulKvRequest(event, '#kv-feedback');
        this.notifyFeedback('#kv-feedback', successful ? 'success' : 'error');
        if (successful) {
          this.closeModal();
          this.refreshTable();
        }
      },
      onDeleteBeforeRequest() {
        this.confirmModal.busy = true;
      },
      onDeleteAfterRequest(event) {
        this.confirmModal.busy = false;
        const successful = this.isSuccessfulKvRequest(event, '#kv-feedback');
        this.notifyFeedback('#kv-feedback', successful ? 'success' : 'error');
        if (successful) {
          this.closeDeleteConfirm();
          this.refreshTable();
        }
      },
    };
  };
})();
