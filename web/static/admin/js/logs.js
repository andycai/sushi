(() => {
  window.logsPage = function logsPage() {
    return {
      logs: [],
      error: null,
      loading: false,
      autoRefresh: false,
      autoPoller: null,
      async init() {
        await this.loadLogs();
        if (window.AdminUI) {
          this.autoPoller = window.AdminUI.createAutoRefresh(() => {
            this.loadLogs();
          }, 5000);
        }
        this.$watch('autoRefresh', (value) => {
          if (this.autoPoller) {
            this.autoPoller.setEnabled(value);
          }
        });
      },
      destroy() {
        if (this.autoPoller) {
          this.autoPoller.dispose();
          this.autoPoller = null;
        }
      },
      badgeTone(level) {
        if (window.AdminUI) {
          return window.AdminUI.levelTone(level);
        }
        return 'info';
      },
      async loadLogs() {
        this.loading = true;
        this.error = null;
        try {
          if (window.AdminUI) {
            const data = await window.AdminUI.fetchJson('/admin/api/logs', {}, 'load logs');
            this.logs = data.logs || [];
          } else {
            const resp = await fetch('/admin/api/logs');
            if (!resp.ok) {
              throw new Error('Failed to load logs');
            }
            const data = await resp.json();
            this.logs = data.logs || [];
          }
        } catch (err) {
          this.error = 'Could not load logs. Ensure /admin/api/logs is available.';
        }
        this.loading = false;
      },
    };
  };
})();
