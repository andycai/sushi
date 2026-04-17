(() => {
  window.cmsPage = function cmsPage() {
    return {
      refreshPages() {
        if (window.AdminUI && typeof window.AdminUI.refreshPartial === 'function') {
          window.AdminUI.refreshPartial({
            url: '/admin/partials/cms/pages/table',
            target: '#cms-page-table',
            errorMessage: 'Unable to refresh page table.',
          });
        }
      },
    };
  };
})();
