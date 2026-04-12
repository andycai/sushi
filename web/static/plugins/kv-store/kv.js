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
    const formFactory =
      window.AdminUI && typeof window.AdminUI.createForm === 'function'
        ? window.AdminUI.createForm
        : fallbackForm;

    return {
      editorModal: modalFactory(() => ({})),
      entryForm: formFactory(makeEntryForm),
      confirmModal: modalFactory(makeDeletePayload),
      get mode() {
        return this.entryForm.values.mode || 'create';
      },
      openCreate() {
        this.confirmModal.hide();
        this.entryForm.reset({ mode: 'create' });
        this.editorModal.show();
      },
      openEdit(key, value) {
        this.confirmModal.hide();
        this.entryForm.reset({
          key,
          value,
          originalKey: key,
          mode: 'edit',
        });
        this.editorModal.show();
      },
      closeModal() {
        this.editorModal.hide();
        this.entryForm.busy = false;
      },
      openDeleteConfirm(key) {
        this.editorModal.hide();
        this.confirmModal.show({ key: key || '' });
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
              message: 'Unable to complete the KV operation.',
            });
          }
        }
      },
      onUpsertBeforeRequest() {
        this.entryForm.busy = true;
      },
      onUpsertAfterRequest(event) {
        this.entryForm.busy = false;
        const successful = Boolean(event?.detail?.successful);
        this.notifyFeedback('#kv-feedback', successful ? 'success' : 'error');
        if (successful) {
          this.closeModal();
          this.triggerRefresh();
        }
      },
      onDeleteBeforeRequest() {
        this.confirmModal.busy = true;
      },
      onDeleteAfterRequest(event) {
        this.confirmModal.busy = false;
        const successful = Boolean(event?.detail?.successful);
        this.notifyFeedback('#kv-feedback', successful ? 'success' : 'error');
        if (successful) {
          this.closeDeleteConfirm();
          this.triggerRefresh();
        }
      },
      triggerRefresh() {
        if (window.AdminUI) {
          window.AdminUI.trigger('kv:refresh');
          return;
        }

        if (window.htmx) {
          window.htmx.trigger(document.body, 'kv:refresh');
        }
      },
    };
  };
})();
