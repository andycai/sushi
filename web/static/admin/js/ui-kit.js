(() => {
  function nowLabel() {
    return new Date().toLocaleTimeString([], {
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
      hour12: false,
    });
  }

  function trigger(eventName, detail) {
    if (window.htmx) {
      window.htmx.trigger(document.body, eventName, detail ?? true);
      return;
    }

    document.body.dispatchEvent(
      new CustomEvent(eventName, {
        bubbles: true,
        detail,
      }),
    );
  }

  function createModal(defaultFactory) {
    const factory =
      typeof defaultFactory === 'function' ? defaultFactory : () => ({});

    return {
      open: false,
      busy: false,
      payload: factory(),
      show(payload = {}) {
        this.open = true;
        this.busy = false;
        this.payload = {
          ...factory(),
          ...payload,
        };
      },
      hide() {
        this.open = false;
        this.busy = false;
        this.payload = factory();
      },
    };
  }

  async function fetchJson(url, options = {}, label = 'request') {
    const response = await fetch(url, options);
    if (!response.ok) {
      throw new Error(`Failed to ${label} (${response.status})`);
    }
    return response.json();
  }

  function createAutoRefresh(onTick, intervalMs = 5000) {
    return {
      enabled: false,
      _timer: null,
      _tick: onTick,
      _interval: intervalMs,
      setEnabled(enabled) {
        this.enabled = Boolean(enabled);
        if (!this.enabled) {
          this.stop();
          return;
        }
        this.start();
      },
      start() {
        this.stop();
        this._timer = window.setInterval(() => {
          this._tick();
        }, this._interval);
      },
      stop() {
        if (this._timer) {
          window.clearInterval(this._timer);
          this._timer = null;
        }
      },
      dispose() {
        this.stop();
      },
    };
  }

  function levelTone(level) {
    const normalized = String(level || '').toUpperCase();
    if (normalized === 'ERROR') {
      return 'danger';
    }
    if (normalized === 'WARN' || normalized === 'WARNING') {
      return 'warning';
    }
    if (normalized === 'DEBUG') {
      return 'muted';
    }
    return 'info';
  }

  window.AdminUI = Object.freeze({
    createAutoRefresh,
    createModal,
    fetchJson,
    levelTone,
    nowLabel,
    trigger,
  });
})();
