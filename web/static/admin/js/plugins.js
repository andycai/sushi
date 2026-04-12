(() => {
  window.pluginsPage = function pluginsPage() {
    return {
      lastUpdated: '',
      markLoaded() {
        const now = new Date();
        this.lastUpdated = now.toLocaleTimeString();
      },
    };
  };
})();
