(() => {
  window.logsPage = function logsPage() {
    return {
      logs: [],
      error: null,
      loading: false,
      autoRefresh: false,
      _interval: null,
      async init() {
        await this.loadLogs();
        this.$watch('autoRefresh', (val) => {
          if (this._interval) {
            clearInterval(this._interval);
            this._interval = null;
          }
          if (val) {
            this._interval = setInterval(() => this.loadLogs(), 5000);
          }
        });
      },
      async loadLogs() {
        this.loading = true;
        this.error = null;
        try {
          const resp = await fetch('/admin/api/logs');
          if (!resp.ok) throw new Error('Failed to fetch logs');
          const data = await resp.json();
          this.logs = data.logs || [];
        } catch (e) {
          this.error = 'Could not load logs. Is the logs endpoint configured?';
        }
        this.loading = false;
      },
    };
  };
})();
