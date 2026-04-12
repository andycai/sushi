(() => {
  window.configPage = function configPage() {
    return {
      config: {},
      loaded: false,
      error: '',
      async init() {
        this.error = '';
        try {
          if (window.AdminUI) {
            this.config = await window.AdminUI.fetchJson(
              '/admin/api/config',
              {},
              'load configuration',
            );
          } else {
            const resp = await fetch('/admin/api/config');
            if (!resp.ok) {
              throw new Error('Failed to load configuration');
            }
            this.config = await resp.json();
          }
        } catch (err) {
          this.config = {};
          this.error = 'Unable to load config from /admin/api/config';
          if (window.AdminUI) {
            window.AdminUI.notify({
              tone: 'danger',
              title: 'Config request failed',
              message: this.error,
            });
          }
        }
        this.loaded = true;
      },
    };
  };
})();
