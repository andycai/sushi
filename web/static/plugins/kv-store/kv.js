(() => {
  window.kvPage = function kvPage() {
    return {
      showModal: false,
      showDeleteModal: false,
      submitting: false,
      deleting: false,
      mode: 'create',
      deleteKey: '',
      form: {
        key: '',
        value: '',
        originalKey: '',
      },
      openCreate() {
        this.mode = 'create';
        this.showModal = true;
        this.showDeleteModal = false;
        this.submitting = false;
        this.form = {
          key: '',
          value: '',
          originalKey: '',
        };
      },
      openEdit(key, value) {
        this.mode = 'edit';
        this.showModal = true;
        this.showDeleteModal = false;
        this.submitting = false;
        this.form = {
          key: key,
          value: value,
          originalKey: key,
        };
      },
      closeModal() {
        this.showModal = false;
        this.submitting = false;
      },
      openDeleteConfirm(key) {
        this.showModal = false;
        this.showDeleteModal = true;
        this.deleting = false;
        this.deleteKey = key || '';
      },
      closeDeleteConfirm() {
        this.showDeleteModal = false;
        this.deleting = false;
        this.deleteKey = '';
      },
      triggerRefresh() {
        if (window.htmx) {
          window.htmx.trigger(document.body, 'kv:refresh');
        }
      },
    };
  };
})();
