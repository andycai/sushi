(() => {
  window.kvPage = function kvPage() {
    return {
      showModal: false,
      submitting: false,
      mode: 'create',
      form: {
        key: '',
        value: '',
        originalKey: '',
      },
      openCreate() {
        this.mode = 'create';
        this.showModal = true;
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
      triggerRefresh() {
        if (window.htmx) {
          window.htmx.trigger(document.body, 'kv:refresh');
        }
      },
    };
  };
})();
