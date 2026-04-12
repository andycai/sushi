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

  window.usersPage = function usersPage() {
    const makeUserForm = () => ({
      username: '',
      email: '',
      password: '',
      role: 'viewer',
    });
    const makeDeletePayload = () => ({
      id: null,
      username: '',
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
        containerSelector: '#users-table-body',
        storageKey: 'admin.users.table.v1',
      }),
      formDrawer: drawerFactory(() => ({})),
      form: formFactory(makeUserForm),
      confirmModal: modalFactory(makeDeletePayload),
      init() {},
      openModal() {
        this.confirmModal.hide();
        this.form.reset();
        this.formDrawer.show();
      },
      closeModal() {
        this.formDrawer.hide();
        this.form.busy = false;
      },
      openDeleteConfirm(id, username) {
        this.formDrawer.hide();
        this.confirmModal.show({
          id,
          username: username || '',
        });
      },
      closeDeleteConfirm() {
        this.confirmModal.hide();
      },
      applySearch() {
        this.table.page = 1;
        this.table.apply('#users-table-body');
      },
      onUsersTableSwap() {
        this.table.onAfterSwap('#users-table-body');
      },
      setPageSize() {
        this.table.setPageSize(this.table.pageSize, '#users-table-body');
      },
      setSortMode() {
        this.table.setSortMode(this.table.sortMode, '#users-table-body');
      },
      prevPage() {
        this.table.prevPage('#users-table-body');
      },
      nextPage() {
        this.table.nextPage('#users-table-body');
      },
      notifyFeedback(selector, fallbackLevel) {
        if (window.AdminUI && typeof window.AdminUI.consumeFeedback === 'function') {
          const consumed = window.AdminUI.consumeFeedback(selector, fallbackLevel);
          if (!consumed && fallbackLevel === 'error') {
            window.AdminUI.notify({
              tone: 'danger',
              title: 'Request failed',
              message: 'Unable to complete the user operation.',
            });
          }
        }
      },
      responseHasTrigger(event, triggerName) {
        if (window.AdminUI && typeof window.AdminUI.hasHxTrigger === 'function') {
          return window.AdminUI.hasHxTrigger(event, triggerName);
        }

        const xhr = event?.detail?.xhr;
        if (!xhr || typeof xhr.getResponseHeader !== 'function') {
          return false;
        }

        const rawValue = xhr.getResponseHeader('HX-Trigger');
        if (!rawValue) {
          return false;
        }

        const value = String(rawValue).trim();
        if (!value) {
          return false;
        }

        if (value.startsWith('{')) {
          try {
            const parsed = JSON.parse(value);
            return Boolean(parsed && parsed[triggerName]);
          } catch (_) {
            return false;
          }
        }

        return value
          .split(',')
          .map((item) => item.trim())
          .includes(triggerName);
      },
      refreshTable() {
        if (window.AdminUI && typeof window.AdminUI.refreshPartial === 'function') {
          window.AdminUI.refreshPartial({
            url: '/admin/partials/users/table',
            target: '#users-table-body',
            onAfterSwap: () => this.onUsersTableSwap(),
            errorMessage: 'Unable to refresh the user list.',
          });
          return;
        }

        fetch('/admin/partials/users/table')
          .then((response) => {
            if (!response.ok) {
              throw new Error(`Failed to refresh users (${response.status})`);
            }
            return response.text();
          })
          .then((html) => {
            const target = document.querySelector('#users-table-body');
            if (!target) {
              return;
            }
            target.innerHTML = html;
            this.onUsersTableSwap();
          })
          .catch(() => {
            if (window.AdminUI) {
              window.AdminUI.notify({
                tone: 'danger',
                title: 'Refresh failed',
                message: 'Unable to refresh the user list.',
              });
            }
          });
      },
      onCreateBeforeRequest() {
        this.form.busy = true;
      },
      onCreateAfterRequest(event) {
        this.form.busy = false;
        const successful = Boolean(event?.detail?.successful);
        this.notifyFeedback('#users-feedback', successful ? 'success' : 'error');
        if (successful) {
          this.closeModal();
          if (!this.responseHasTrigger(event, 'users:refresh')) {
            this.refreshTable();
          }
        }
      },
      onDeleteBeforeRequest() {
        this.confirmModal.busy = true;
      },
      onDeleteAfterRequest(event) {
        this.confirmModal.busy = false;
        const successful = Boolean(event?.detail?.successful);
        this.notifyFeedback('#users-feedback', successful ? 'success' : 'error');
        if (successful) {
          this.closeDeleteConfirm();
          if (!this.responseHasTrigger(event, 'users:refresh')) {
            this.refreshTable();
          }
        }
      },
    };
  };
})();
