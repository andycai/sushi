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
      }),
      formDrawer: drawerFactory(() => ({})),
      form: formFactory(makeUserForm),
      confirmModal: modalFactory(makeDeletePayload),
      init() {
        this.applySearch();
      },
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
        this.table.apply('#users-table-body');
      },
      onUsersTableSwap() {
        this.table.onAfterSwap('#users-table-body');
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
      onCreateBeforeRequest() {
        this.form.busy = true;
      },
      onCreateAfterRequest(event) {
        this.form.busy = false;
        const successful = Boolean(event?.detail?.successful);
        this.notifyFeedback('#users-feedback', successful ? 'success' : 'error');
        if (successful) {
          this.closeModal();
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
        }
      },
    };
  };
})();
