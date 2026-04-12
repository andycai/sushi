(() => {
  window.usersPage = function usersPage() {
    return {
      showModal: false,
      submitting: false,
      newUser: {
        username: '',
        email: '',
        password: '',
        role: 'viewer',
      },
      openModal() {
        this.showModal = true;
        this.submitting = false;
        this.newUser = {
          username: '',
          email: '',
          password: '',
          role: 'viewer',
        };
      },
      closeModal() {
        this.showModal = false;
        this.submitting = false;
      },
    };
  };
})();
