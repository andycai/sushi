(() => {
  async function readJson(resp) {
    const text = await resp.text();
    if (!text) return null;
    try {
      return JSON.parse(text);
    } catch (_) {
      return null;
    }
  }

  async function requestJson(url, options) {
    const resp = await fetch(url, options);
    const data = await readJson(resp);
    if (!resp.ok) {
      const message = data && typeof data.error === 'string'
        ? data.error
        : `Request failed (${resp.status})`;
      throw new Error(message);
    }
    if (data && typeof data.error === 'string') {
      throw new Error(data.error);
    }
    return data;
  }

  window.kvPage = function kvPage() {
    return {
      items: [],
      loading: true,
      error: null,
      showAddModal: false,
      showEditModal: false,
      form: { key: '', value: '' },
      async init() {
        await this.loadItems();
        this.loading = false;
      },
      async loadItems() {
        this.loading = true;
        try {
          const data = await requestJson('/api/kv');
          this.items = Array.isArray(data) ? data : [];
          this.error = null;
        } catch (e) {
          this.error = 'Could not load KV entries: ' + (e instanceof Error ? e.message : 'unknown error');
        }
        this.loading = false;
      },
      editItem(item) {
        this.form = { key: item.key, value: item.value };
        this.showEditModal = true;
      },
      async deleteItem(key) {
        if (!confirm('Delete "' + key + '"?')) return;
        try {
          await requestJson('/api/kv/' + encodeURIComponent(key), { method: 'DELETE' });
          await this.loadItems();
        } catch (e) {
          this.error = 'Failed to delete: ' + (e instanceof Error ? e.message : 'unknown error');
        }
      },
      async saveItem() {
        try {
          if (this.showAddModal) {
            await requestJson('/api/kv', {
              method: 'POST',
              headers: { 'Content-Type': 'application/json' },
              body: JSON.stringify(this.form),
            });
          } else {
            await requestJson('/api/kv/' + encodeURIComponent(this.form.key), {
              method: 'PUT',
              headers: { 'Content-Type': 'application/json' },
              body: JSON.stringify({ value: this.form.value }),
            });
          }
          this.closeModal();
          await this.loadItems();
        } catch (e) {
          this.error = 'Failed to save: ' + (e instanceof Error ? e.message : 'unknown error');
        }
      },
      closeModal() {
        this.showAddModal = false;
        this.showEditModal = false;
        this.form = { key: '', value: '' };
      },
    };
  };
})();
