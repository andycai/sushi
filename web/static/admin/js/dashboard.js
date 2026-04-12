(() => {
  window.adminApp = function adminApp() {
    return {
      pulse: window.AdminUI ? window.AdminUI.nowLabel() : '--:--:--',
      stats: {
        plugins: 0,
        users: 0,
        uptime: 'online',
      },
      refreshPulse() {
        if (window.AdminUI) {
          this.pulse = window.AdminUI.nowLabel();
        }
      },
      init() {
        this.refreshPulse();
      },
    };
  };
})();
