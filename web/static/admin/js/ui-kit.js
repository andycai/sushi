(() => {
  const TOAST_ROOT_ID = 'admin-ui-toast-root';

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

  function createDrawer(defaultFactory) {
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

  function createForm(defaultFactory) {
    const factory =
      typeof defaultFactory === 'function' ? defaultFactory : () => ({});

    return {
      busy: false,
      values: factory(),
      errors: {},
      reset(values = {}) {
        this.busy = false;
        this.errors = {};
        this.values = {
          ...factory(),
          ...values,
        };
      },
      setErrors(errors = {}) {
        this.errors = { ...errors };
      },
      clearErrors() {
        this.errors = {};
      },
    };
  }

  function createDataTable({
    containerSelector = '',
    rowSelector = 'tr[data-row-search]',
    rowPredicate = null,
    storageKey = '',
    pageSizeOptions = [10, 20, 50],
    initialPageSize = 10,
  } = {}) {
    const normalizedPageSize = Number(initialPageSize);
    const defaultPageSize =
      Number.isFinite(normalizedPageSize) && normalizedPageSize > 0
        ? normalizedPageSize
        : 10;

    function normalizedText(value) {
      return String(value || '').toLowerCase().trim();
    }

    function localStorageAvailable() {
      try {
        return (
          typeof window !== 'undefined' &&
          typeof window.localStorage !== 'undefined'
        );
      } catch (_) {
        return false;
      }
    }

    function readState() {
      if (!storageKey || !localStorageAvailable()) {
        return {};
      }

      try {
        const raw = window.localStorage.getItem(storageKey);
        if (!raw) {
          return {};
        }
        const parsed = JSON.parse(raw);
        if (parsed && typeof parsed === 'object') {
          return parsed;
        }
      } catch (_) {}
      return {};
    }

    function toPositiveNumber(value, fallback) {
      const parsed = Number(value);
      if (!Number.isFinite(parsed) || parsed <= 0) {
        return fallback;
      }
      return parsed;
    }

    const persisted = readState();
    const initialState = {
      query: typeof persisted.query === 'string' ? persisted.query : '',
      sortMode:
        typeof persisted.sortMode === 'string' ? persisted.sortMode : 'default',
      page: toPositiveNumber(persisted.page, 1),
      pageSize: toPositiveNumber(persisted.pageSize, defaultPageSize),
      meta:
        persisted.meta && typeof persisted.meta === 'object'
          ? { ...persisted.meta }
          : {},
    };

    return {
      query: initialState.query,
      sortMode: initialState.sortMode,
      page: initialState.page,
      pageSize: initialState.pageSize,
      pageSizeOptions: Array.from(pageSizeOptions),
      rowPredicate,
      meta: initialState.meta,
      totalRows: 0,
      filteredRows: 0,
      visibleRows: 0,
      totalPages: 1,
      emptyFiltered: false,
      _saveState() {
        if (!storageKey || !localStorageAvailable()) {
          return;
        }

        const payload = {
          query: this.query,
          sortMode: this.sortMode,
          page: this.page,
          pageSize: this.pageSize,
          meta: this.meta || {},
        };

        try {
          window.localStorage.setItem(storageKey, JSON.stringify(payload));
        } catch (_) {}
      },
      clearState() {
        if (!storageKey || !localStorageAvailable()) {
          return;
        }

        try {
          window.localStorage.removeItem(storageKey);
        } catch (_) {}
      },
      _sortRows(rows) {
        const sorted = [...rows];
        if (this.sortMode === 'alpha_asc') {
          sorted.sort((left, right) =>
            normalizedText(left.dataset.rowSort || left.dataset.rowSearch).localeCompare(
              normalizedText(right.dataset.rowSort || right.dataset.rowSearch),
            ),
          );
          return sorted;
        }

        if (this.sortMode === 'alpha_desc') {
          sorted.sort((left, right) =>
            normalizedText(right.dataset.rowSort || right.dataset.rowSearch).localeCompare(
              normalizedText(left.dataset.rowSort || left.dataset.rowSearch),
            ),
          );
          return sorted;
        }

        sorted.sort(
          (left, right) =>
            Number(left.dataset.rowIndex || 0) - Number(right.dataset.rowIndex || 0),
        );
        return sorted;
      },
      apply(containerOrSelector = containerSelector) {
        const container = resolveContainer(containerOrSelector);
        if (!container) {
          this.totalRows = 0;
          this.filteredRows = 0;
          this.visibleRows = 0;
          this.totalPages = 1;
          this.emptyFiltered = false;
          this.page = 1;
          return;
        }

        const rows = Array.from(container.querySelectorAll(rowSelector));
        rows.forEach((row, index) => {
          if (!row.dataset.rowIndex) {
            row.dataset.rowIndex = String(index);
          }
        });

        const q = normalizedText(this.query);
        const matchedRows = rows.filter((row) => {
          const matchedByPredicate =
            typeof this.rowPredicate === 'function'
              ? this.rowPredicate(row, this)
              : true;
          if (!matchedByPredicate) {
            return false;
          }

          const haystack = normalizedText(
            row.dataset.rowSearch || row.textContent || '',
          );
          return !q || haystack.includes(q);
        });
        const sortedRows = this._sortRows(matchedRows);
        sortedRows.forEach((row) => container.appendChild(row));

        this.totalRows = rows.length;
        this.filteredRows = sortedRows.length;
        const normalizedCurrentPageSize = Number(this.pageSize);
        this.pageSize =
          Number.isFinite(normalizedCurrentPageSize) && normalizedCurrentPageSize > 0
            ? normalizedCurrentPageSize
            : defaultPageSize;

        this.totalPages = Math.max(1, Math.ceil(this.filteredRows / this.pageSize));
        this.page = Math.max(1, Math.min(this.page, this.totalPages));

        const start = (this.page - 1) * this.pageSize;
        const end = start + this.pageSize;
        let visible = 0;
        const matchedSet = new Set(sortedRows);

        rows.forEach((row) => {
          if (!matchedSet.has(row)) {
            row.style.display = 'none';
          }
        });

        sortedRows.forEach((row, index) => {
          const show = index >= start && index < end;
          row.style.display = show ? '' : 'none';
          if (show) {
            visible += 1;
          }
        });

        this.visibleRows = visible;
        this.emptyFiltered = q.length > 0 && this.filteredRows === 0;
        this._saveState();
      },
      onAfterSwap(containerOrSelector = containerSelector) {
        this.apply(containerOrSelector);
      },
      reset(containerOrSelector = containerSelector) {
        this.query = '';
        this.sortMode = 'default';
        this.page = 1;
        this.pageSize = defaultPageSize;
        this.meta = {};
        this.clearState();
        this.apply(containerOrSelector);
      },
      setPageSize(size, containerOrSelector = containerSelector) {
        const parsed = Number(size);
        if (Number.isFinite(parsed) && parsed > 0) {
          this.pageSize = parsed;
        }
        this.page = 1;
        this.apply(containerOrSelector);
      },
      setSortMode(mode, containerOrSelector = containerSelector) {
        this.sortMode = mode || 'default';
        this.page = 1;
        this.apply(containerOrSelector);
      },
      prevPage(containerOrSelector = containerSelector) {
        if (this.page <= 1) {
          return;
        }
        this.page -= 1;
        this.apply(containerOrSelector);
      },
      nextPage(containerOrSelector = containerSelector) {
        if (this.page >= this.totalPages) {
          return;
        }
        this.page += 1;
        this.apply(containerOrSelector);
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

  function ensureToastRoot() {
    if (typeof document === 'undefined') {
      return null;
    }

    const existing = document.getElementById(TOAST_ROOT_ID);
    if (existing) {
      return existing;
    }

    const root = document.createElement('div');
    root.id = TOAST_ROOT_ID;
    root.className = 'ui-toast-stack';
    document.body.appendChild(root);
    return root;
  }

  function dismissToast(node) {
    if (!node || !node.parentNode) {
      return;
    }

    node.classList.add('closing');
    window.setTimeout(() => {
      if (node.parentNode) {
        node.parentNode.removeChild(node);
      }
    }, 180);
  }

  function notify({
    tone = 'info',
    title = '',
    message = '',
    timeoutMs = 3400,
  }) {
    if (!message) {
      return;
    }

    const root = ensureToastRoot();
    if (!root) {
      return;
    }

    const node = document.createElement('div');
    node.className = `ui-toast ${tone}`;

    const close = document.createElement('button');
    close.type = 'button';
    close.className = 'ui-toast-close';
    close.textContent = 'x';
    close.addEventListener('click', () => dismissToast(node));

    if (title) {
      const titleNode = document.createElement('div');
      titleNode.className = 'ui-toast-title';
      titleNode.textContent = title;
      node.appendChild(titleNode);
    }

    const messageNode = document.createElement('div');
    messageNode.className = 'ui-toast-message';
    messageNode.textContent = message;
    node.appendChild(messageNode);
    node.appendChild(close);
    root.appendChild(node);

    window.setTimeout(() => dismissToast(node), timeoutMs);
  }

  function resolveContainer(containerOrSelector) {
    if (!containerOrSelector) {
      return null;
    }

    if (typeof containerOrSelector === 'string') {
      return document.querySelector(containerOrSelector);
    }

    return containerOrSelector;
  }

  function readFeedback(containerOrSelector, fallbackLevel = 'info') {
    const container = resolveContainer(containerOrSelector);
    if (!container) {
      return null;
    }

    const flash = container.querySelector('[data-ui-flash]');
    if (!flash) {
      return null;
    }

    const rawLevel = flash.dataset.level || fallbackLevel;
    const tone = levelTone(rawLevel);
    const message = (flash.dataset.message || flash.textContent || '').trim();
    if (!message) {
      return null;
    }

    return {
      tone,
      level: String(rawLevel || '').toLowerCase(),
      message,
    };
  }

  function isErrorFeedback(containerOrSelector, fallbackLevel = 'info') {
    const feedback = readFeedback(containerOrSelector, fallbackLevel);
    if (!feedback) {
      return false;
    }
    return feedback.tone === 'danger' || feedback.level === 'error';
  }

  function consumeFeedback(containerOrSelector, fallbackLevel = 'info') {
    const feedback = readFeedback(containerOrSelector, fallbackLevel);
    if (!feedback) {
      return null;
    }

    notify({
      tone: feedback.tone,
      title:
        feedback.tone === 'danger'
          ? 'Request failed'
          : feedback.tone === 'warning'
            ? 'Check required'
            : 'Request completed',
      message: feedback.message,
    });

    return feedback;
  }

  function hasHxTrigger(event, triggerName) {
    if (!triggerName) {
      return false;
    }

    const xhr = event?.detail?.xhr;
    if (!xhr || typeof xhr.getResponseHeader !== 'function') {
      return false;
    }

    const rawValue = xhr.getResponseHeader('HX-Trigger');
    if (!rawValue) {
      return false;
    }

    const value = String(rawValue).trim();
    if (!value) {
      return false;
    }

    if (value.startsWith('{')) {
      try {
        const parsed = JSON.parse(value);
        return Boolean(parsed && parsed[triggerName]);
      } catch (_) {
        return false;
      }
    }

    return value
      .split(',')
      .map((item) => item.trim())
      .includes(triggerName);
  }

  function refreshPartial({
    url = '',
    target = '',
    onAfterSwap = null,
    errorTitle = 'Refresh failed',
    errorMessage = 'Unable to refresh data.',
  } = {}) {
    if (!url || !target) {
      return;
    }

    if (window.htmx && typeof window.htmx.ajax === 'function') {
      window.htmx.ajax('GET', url, {
        target,
        swap: 'innerHTML',
      });
      return;
    }

    fetch(url)
      .then((response) => {
        if (!response.ok) {
          throw new Error(`Failed to refresh partial (${response.status})`);
        }
        return response.text();
      })
      .then((html) => {
        const targetNode = resolveContainer(target);
        if (!targetNode) {
          return;
        }
        targetNode.innerHTML = html;
        if (typeof onAfterSwap === 'function') {
          onAfterSwap();
        }
      })
      .catch(() => {
        notify({
          tone: 'danger',
          title: errorTitle,
          message: errorMessage,
        });
      });
  }

  window.AdminUI = Object.freeze({
    hasHxTrigger,
    isErrorFeedback,
    readFeedback,
    refreshPartial,
    consumeFeedback,
    createAutoRefresh,
    createDataTable,
    createDrawer,
    createForm,
    createModal,
    fetchJson,
    levelTone,
    notify,
    nowLabel,
    trigger,
  });
})();
