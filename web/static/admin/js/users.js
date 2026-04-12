(() => {
  function fallbackModal(factory) {
    const seed = factory();
    return {
      open: false,
      busy: false,
      payload: seed,
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

    return {
      formModal: modalFactory(makeUserForm),
      confirmModal: modalFactory(makeDeletePayload),
      openModal() {
        this.confirmModal.hide();
        this.formModal.show();
      },
      closeModal() {
        this.formModal.hide();
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
    };
  };
})();
