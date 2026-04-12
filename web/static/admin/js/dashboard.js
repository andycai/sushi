(() => {
  window.adminApp = function adminApp() {
    Alpine.store('stats', { plugins: 0, users: 0, uptime: '-' });
    return {};
  };
})();
