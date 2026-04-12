(() => {
  window.usersPage = function usersPage() {
    return {
      showModal: false,
      submitting: false,
      showDeleteModal: false,
      deleting: false,
      deleteCandidate: {
        id: null,
        username: '',
      },
      newUser: {
        username: '',
        email: '',
        password: '',
        role: 'viewer',
      },
      openModal() {
        this.showModal = true;
        this.submitting = false;
        this.showDeleteModal = false;
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
      openDeleteConfirm(id, username) {
        this.showModal = false;
        this.showDeleteModal = true;
        this.deleting = false;
        this.deleteCandidate = {
          id: id,
          username: username || '',
        };
      },
      closeDeleteConfirm() {
        this.showDeleteModal = false;
        this.deleting = false;
        this.deleteCandidate = {
          id: null,
          username: '',
        };
      },
    };
  };
})();
