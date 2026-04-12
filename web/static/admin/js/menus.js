(() => {
  window.menuManagement = function menuManagement() {
    return {
      ...window.adminMenu(),

      showForm: false,
      editingItem: null,
      form: {
        label: '',
        icon: '',
        route: '',
        parent_id: '',
        is_hidden: false,
      },
      showDeleteConfirm: false,
      deleteTargetId: null,

      topItems() {
        return this.menuItems.filter(i => !i.parent_id);
      },

      getChildren(parentId) {
        return this.menuItems.filter(i => i.parent_id === parentId);
      },

      openForm(item = null) {
        this.editingItem = item;
        if (item) {
          this.form = { ...item, parent_id: item.parent_id || '' };
        } else {
          this.form = { label: '', icon: '', route: '', parent_id: '', is_hidden: false };
        }
        this.showForm = true;
      },

      closeForm() {
        this.showForm = false;
        this.editingItem = null;
      },

      async saveItem() {
        const method = this.editingItem ? 'PUT' : 'POST';
        const url = this.editingItem
          ? `/admin/api/menu/${this.editingItem.id}`
          : '/admin/api/menu';

        const payload = { ...this.form };
        if (payload.parent_id === '') payload.parent_id = null;

        await fetch(url, {
          method,
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(payload),
        });

        this.closeForm();
        await this.loadMenu();
      },

      async deleteItem(id) {
        this.deleteTargetId = id;
        this.showDeleteConfirm = true;
      },

      async confirmDelete() {
        if (!this.deleteTargetId) return;
        await fetch(`/admin/api/menu/${this.deleteTargetId}`, { method: 'DELETE' });
        this.showDeleteConfirm = false;
        this.deleteTargetId = null;
        await this.loadMenu();
      },

      cancelDelete() {
        this.showDeleteConfirm = false;
        this.deleteTargetId = null;
      },
    };
  };
})();