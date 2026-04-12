(() => {
  window.pluginsPage = function pluginsPage() {
    return {
      lastUpdated: '',
      markLoaded() {
        this.lastUpdated = window.AdminUI ? window.AdminUI.nowLabel() : new Date().toLocaleTimeString();
      },
    };
  };
})();
