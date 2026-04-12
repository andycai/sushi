(() => {
  window.pluginsPage = function pluginsPage() {
    return {
      plugins: [],
      async init() {
        try {
          const resp = await fetch('/admin/api/plugins');
          if (!resp.ok) throw new Error('Failed to fetch plugins');
          this.plugins = await resp.json();
        } catch (e) {
          this.plugins = [];
        }
      },
    };
  };
})();
