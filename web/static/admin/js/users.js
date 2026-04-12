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
    const formFactory =
      window.AdminUI && typeof window.AdminUI.createForm === 'function'
        ? window.AdminUI.createForm
        : fallbackForm;

    return {
      formModal: modalFactory(() => ({})),
      form: formFactory(makeUserForm),
      confirmModal: modalFactory(makeDeletePayload),
      openModal() {
        this.confirmModal.hide();
        this.form.reset();
        this.formModal.show();
      },
      closeModal() {
        this.formModal.hide();
        this.form.busy = false;
      },
      openDeleteConfirm(id, username) {
        this.formModal.hide();
        this.confirmModal.show({
          id,
          username: username || '',
        });
      },
      closeDeleteConfirm() {
        this.confirmModal.hide();
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
