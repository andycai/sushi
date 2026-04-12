(() => {
  window.loginPage = function loginPage() {
    return {
      submitting: false,
      showPassword: false,
      onSubmitStart() {
        this.submitting = true;
      },
      onSubmitEnd(event) {
        this.submitting = false;
        const successful = Boolean(event?.detail?.successful);
        if (!successful && window.AdminUI) {
          window.AdminUI.consumeFeedback('#login-error', 'error');
        }
      },
    };
  };
})();
