(() => {
  function fallbackModal(factory) {
    return {
      open: false,
      busy: false,
      payload: factory(),
      show(payload = {}) {
        this.open = true;
        this.busy = false;
        this.payload = { ...factory(), ...payload };
      },
      hide() {
        this.open = false;
        this.busy = false;
        this.payload = factory();
      },
    };
  }

  window.kvPage = function kvPage() {
    const makeFormPayload = () => ({
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

    return {
      editorModal: modalFactory(makeFormPayload),
      confirmModal: modalFactory(makeDeletePayload),
      get mode() {
        return this.editorModal.payload.mode || 'create';
      },
      openCreate() {
        this.confirmModal.hide();
        this.editorModal.show({ mode: 'create' });
      },
      openEdit(key, value) {
        this.confirmModal.hide();
        this.editorModal.show({
          key,
          value,
          originalKey: key,
          mode: 'edit',
        });
      },
      closeModal() {
        this.editorModal.hide();
      },
      openDeleteConfirm(key) {
        this.editorModal.hide();
        this.confirmModal.show({ key: key || '' });
      },
      closeDeleteConfirm() {
        this.confirmModal.hide();
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
