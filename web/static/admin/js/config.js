(() => {
  window.configPage = function configPage() {
    return {
      config: {},
      loaded: false,
      async init() {
        try {
          const resp = await fetch('/admin/api/config');
          if (resp.ok) {
            this.config = await resp.json();
          } else {
            throw new Error('Failed to load config');
          }
        } catch (e) {
          this.config = {};
        }
        this.loaded = true;
      },
    };
  };
})();
