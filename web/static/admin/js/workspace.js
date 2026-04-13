(() => {
  const STORAGE_KEY = 'admin.workspace.tabs.v1';
  const DASHBOARD_PATH = '/admin/';
  const DASHBOARD_TITLE = 'Dashboard';
  const WORKSPACE_EVENT = 'admin:workspace:change';

  const state = {
    initialized: false,
    enabled: false,
    rootEl: null,
    tabsEl: null,
    panelEl: null,
    tabs: [],
    activePath: DASHBOARD_PATH,
  };

  function normalizePath(rawPath) {
    if (!rawPath || typeof rawPath !== 'string') {
      return null;
    }

    let url;
    try {
      url = new URL(rawPath, window.location.origin);
    } catch (_) {
      return null;
    }

    let path = url.pathname || '';
    if (path === '/admin') {
      path = DASHBOARD_PATH;
    }

    if (!path.startsWith('/admin/')) {
      return null;
    }

    if (path !== DASHBOARD_PATH) {
      path = path.replace(/\/+$/, '');
      if (!path) {
        path = DASHBOARD_PATH;
      }
    }

    return path;
  }

  function isAdminWorkspacePage(path) {
    return path === DASHBOARD_PATH || (typeof path === 'string' && path.startsWith('/admin/'));
  }

  function supportsWorkspace() {
    const currentPath = normalizePath(window.location.pathname);
    if (!isAdminWorkspacePage(currentPath)) {
      return false;
    }

    const rootEl = document.getElementById('admin-workspace');
    const tabsEl = document.getElementById('admin-workspace-tabs');
    const panelEl = document.getElementById('admin-workspace-panel');
    if (!rootEl || !tabsEl || !panelEl) {
      return false;
    }

    state.rootEl = rootEl;
    state.tabsEl = tabsEl;
    state.panelEl = panelEl;
    return true;
  }

  function canUseStorage() {
    try {
      return typeof window.localStorage !== 'undefined';
    } catch (_) {
      return false;
    }
  }

  function getStorage() {
    if (!canUseStorage()) {
      return null;
    }

    try {
      const raw = window.localStorage.getItem(STORAGE_KEY);
      if (!raw) {
        return null;
      }
      const parsed = JSON.parse(raw);
      if (!parsed || typeof parsed !== 'object') {
        return null;
      }
      return parsed;
    } catch (_) {
      return null;
    }
  }

  function setStorage() {
    if (!canUseStorage()) {
      return;
    }

    const payload = {
      tabs: state.tabs.map((tab) => ({
        path: tab.path,
        title: tab.title,
      })),
      activePath: state.activePath,
    };

    try {
      window.localStorage.setItem(STORAGE_KEY, JSON.stringify(payload));
    } catch (_) {}
  }

  function moduleFromPath(path) {
    if (path === DASHBOARD_PATH) {
      return 'dashboard';
    }

    const value = path.replace(/^\/admin\/?/, '');
    if (!value) {
      return 'dashboard';
    }

    return value
      .split('/')
      .filter(Boolean)
      .map((part) => encodeURIComponent(part))
      .join('/');
  }

  function workspaceUrl(path) {
    return `/admin/workspace/${moduleFromPath(path)}`;
  }

  function titleFromPath(path) {
    if (path === DASHBOARD_PATH) {
      return DASHBOARD_TITLE;
    }

    const leaf = path
      .replace(/^\/admin\/?/, '')
      .split('/')
      .filter(Boolean)
      .slice(-1)[0];

    if (!leaf) {
      return DASHBOARD_TITLE;
    }

    return leaf
      .split(/[-_]/)
      .filter(Boolean)
      .map((segment) => segment.charAt(0).toUpperCase() + segment.slice(1))
      .join(' ');
  }

  function getTab(path) {
    return state.tabs.find((tab) => tab.path === path) || null;
  }

  function ensureDashboardTab() {
    const existing = getTab(DASHBOARD_PATH);
    if (!existing) {
      state.tabs.unshift({
        path: DASHBOARD_PATH,
        title: DASHBOARD_TITLE,
        closable: false,
      });
      return;
    }

    existing.title = DASHBOARD_TITLE;
    existing.closable = false;
    state.tabs = [existing, ...state.tabs.filter((tab) => tab.path !== DASHBOARD_PATH)];
  }

  function upsertTab(path, title) {
    const normalizedPath = normalizePath(path);
    if (!normalizedPath) {
      return null;
    }

    const existing = getTab(normalizedPath);
    if (existing) {
      if (title && title.trim()) {
        existing.title = title.trim();
      }
      existing.closable = normalizedPath !== DASHBOARD_PATH;
      return existing;
    }

    const tab = {
      path: normalizedPath,
      title:
        normalizedPath === DASHBOARD_PATH
          ? DASHBOARD_TITLE
          : title && title.trim()
            ? title.trim()
            : titleFromPath(normalizedPath),
      closable: normalizedPath !== DASHBOARD_PATH,
    };
    state.tabs.push(tab);
    return tab;
  }

  function getPane(path) {
    return Array.from(
      state.panelEl.querySelectorAll('[data-workspace-pane-path]'),
    ).find((pane) => pane.dataset.workspacePanePath === path) || null;
  }

  function createPane(path) {
    const pane = document.createElement('section');
    pane.className = 'admin-workspace-pane';
    pane.dataset.workspacePanePath = path;
    pane.dataset.loaded = 'false';
    pane.hidden = true;
    state.panelEl.appendChild(pane);
    return pane;
  }

  function ensurePane(path) {
    const existing = getPane(path);
    if (existing) {
      return existing;
    }
    return createPane(path);
  }

  function bootstrapInitialPane(currentPath) {
    if (getPane(currentPath)) {
      return;
    }

    const pane = document.createElement('section');
    pane.className = 'admin-workspace-pane is-active';
    pane.dataset.workspacePanePath = currentPath;
    pane.dataset.loaded = 'true';
    pane.hidden = false;

    while (state.panelEl.firstChild) {
      pane.appendChild(state.panelEl.firstChild);
    }

    state.panelEl.appendChild(pane);
  }

  function emitChange() {
    window.dispatchEvent(
      new CustomEvent(WORKSPACE_EVENT, {
        detail: {
          path: state.activePath,
          tabs: state.tabs.map((tab) => ({
            path: tab.path,
            title: tab.title,
            closable: tab.closable,
          })),
        },
      }),
    );
  }

  function renderTabs() {
    if (!state.tabsEl) {
      return;
    }

    state.tabsEl.innerHTML = '';

    state.tabs.forEach((tab) => {
      const item = document.createElement('div');
      item.className = `admin-workspace-tab${
        tab.path === state.activePath ? ' is-active' : ''
      }`;
      item.dataset.path = tab.path;

      const trigger = document.createElement('button');
      trigger.type = 'button';
      trigger.className = 'admin-workspace-tab-trigger';
      trigger.textContent = tab.title;
      trigger.title = tab.path;
      trigger.addEventListener('click', () => {
        activateTab(tab.path, { pushHistory: true, forceReload: false });
      });
      item.appendChild(trigger);

      if (tab.closable) {
        const closeButton = document.createElement('button');
        closeButton.type = 'button';
        closeButton.className = 'admin-workspace-tab-close';
        closeButton.setAttribute('aria-label', `Close ${tab.title}`);
        closeButton.textContent = '×';
        closeButton.addEventListener('click', (event) => {
          event.preventDefault();
          event.stopPropagation();
          closeTab(tab.path);
        });
        item.appendChild(closeButton);
      }

      state.tabsEl.appendChild(item);
    });
  }

  function showOnlyActivePane() {
    const panes = state.panelEl.querySelectorAll('[data-workspace-pane-path]');
    panes.forEach((pane) => {
      const isActive = pane.dataset.workspacePanePath === state.activePath;
      pane.hidden = !isActive;
      pane.classList.toggle('is-active', isActive);
    });
  }

  function renderLoadError(pane, path) {
    pane.dataset.loaded = 'false';
    pane.classList.remove('is-loading');
    pane.innerHTML =
      '<div class="admin-workspace-error">' +
      '<strong>Unable to load this workspace tab.</strong>' +
      `<span>Open <a href="${path}">${path}</a> to continue.</span>` +
      '</div>';
  }

  function loadPane(path) {
    const pane = ensurePane(path);
    if (!pane) {
      return;
    }

    if (!window.htmx || typeof window.htmx.ajax !== 'function') {
      window.location.href = path;
      return;
    }

    pane.classList.add('is-loading');
    pane.innerHTML = '<div class="admin-workspace-loading">Loading workspace tab...</div>';

    const cleanup = (afterSwapHandler, errorHandler) => {
      pane.removeEventListener('htmx:afterSwap', afterSwapHandler);
      pane.removeEventListener('htmx:responseError', errorHandler);
      pane.removeEventListener('htmx:sendError', errorHandler);
    };

    const afterSwapHandler = (event) => {
      if (event.target !== pane) {
        return;
      }

      pane.classList.remove('is-loading');
      pane.dataset.loaded = 'true';
      cleanup(afterSwapHandler, errorHandler);
    };

    const errorHandler = (event) => {
      if (event.target !== pane) {
        return;
      }

      cleanup(afterSwapHandler, errorHandler);
      renderLoadError(pane, path);
    };

    const requestPane = () => {
      pane.addEventListener('htmx:afterSwap', afterSwapHandler);
      pane.addEventListener('htmx:responseError', errorHandler);
      pane.addEventListener('htmx:sendError', errorHandler);

      try {
        window.htmx.ajax('GET', workspaceUrl(path), {
          target: pane,
          swap: 'innerHTML',
        });
      } catch (_) {
        cleanup(afterSwapHandler, errorHandler);
        window.location.href = path;
      }
    };

    const moduleLoader =
      window.AdminModuleLoader &&
      typeof window.AdminModuleLoader.loadForPath === 'function'
        ? window.AdminModuleLoader
        : null;

    if (!moduleLoader) {
      requestPane();
      return;
    }

    Promise.resolve(moduleLoader.loadForPath(path))
      .catch((err) => {
        console.warn('Failed to preload workspace module script:', err);
        return false;
      })
      .finally(() => {
        requestPane();
      });
  }

  function syncHistory(path, pushHistory) {
    const payload = {
      __adminWorkspace: true,
      path,
    };

    if (window.location.pathname === path) {
      window.history.replaceState(payload, '', path);
      return;
    }

    if (pushHistory) {
      window.history.pushState(payload, '', path);
      return;
    }

    window.history.replaceState(payload, '', path);
  }

  function activateTab(path, { pushHistory = false, forceReload = false } = {}) {
    const normalizedPath = normalizePath(path);
    if (!normalizedPath) {
      return false;
    }

    const tab = upsertTab(normalizedPath);
    if (!tab) {
      return false;
    }

    state.activePath = normalizedPath;
    ensureDashboardTab();

    const pane = ensurePane(normalizedPath);
    renderTabs();
    showOnlyActivePane();

    if (forceReload || pane.dataset.loaded !== 'true') {
      loadPane(normalizedPath);
    }

    setStorage();
    syncHistory(normalizedPath, pushHistory);
    emitChange();
    return true;
  }

  function closeTab(path) {
    const normalizedPath = normalizePath(path);
    if (!normalizedPath || normalizedPath === DASHBOARD_PATH) {
      return false;
    }

    const index = state.tabs.findIndex((tab) => tab.path === normalizedPath);
    if (index === -1) {
      return false;
    }

    const closingActive = state.activePath === normalizedPath;
    state.tabs.splice(index, 1);

    const pane = getPane(normalizedPath);
    if (pane) {
      pane.remove();
    }

    ensureDashboardTab();
    renderTabs();

    if (closingActive) {
      const nextTab = state.tabs[index] || state.tabs[index - 1] || state.tabs[0];
      if (nextTab) {
        activateTab(nextTab.path, { pushHistory: true, forceReload: false });
        return true;
      }
    }

    showOnlyActivePane();
    setStorage();
    emitChange();
    return true;
  }

  function restoreTabs(currentPath) {
    const stored = getStorage();
    const seen = new Set();
    const restored = [];

    if (stored && Array.isArray(stored.tabs)) {
      stored.tabs.forEach((entry) => {
        const normalizedPath = normalizePath(String(entry?.path || ''));
        if (!normalizedPath || seen.has(normalizedPath)) {
          return;
        }
        seen.add(normalizedPath);
        restored.push({
          path: normalizedPath,
          title:
            typeof entry?.title === 'string' && entry.title.trim()
              ? entry.title.trim()
              : titleFromPath(normalizedPath),
          closable: normalizedPath !== DASHBOARD_PATH,
        });
      });
    }

    state.tabs = restored;
    ensureDashboardTab();
    upsertTab(currentPath);
  }

  function handlePopstate() {
    if (!state.enabled) {
      return;
    }

    const nextPath = normalizePath(window.location.pathname);
    if (!nextPath) {
      return;
    }

    activateTab(nextPath, { pushHistory: false, forceReload: false });
  }

  function initWorkspace() {
    if (state.initialized) {
      return;
    }
    state.initialized = true;

    if (!supportsWorkspace()) {
      return;
    }

    const currentPath = normalizePath(window.location.pathname) || DASHBOARD_PATH;
    restoreTabs(currentPath);
    bootstrapInitialPane(currentPath);

    state.enabled = true;
    state.rootEl.classList.add('is-enabled');

    ensureDashboardTab();
    activateTab(currentPath, { pushHistory: false, forceReload: false });
    window.addEventListener('popstate', handlePopstate);
  }

  const api = {
    openTab(path, options = {}) {
      if (!state.enabled) {
        return false;
      }

      const normalizedPath = normalizePath(path);
      if (!normalizedPath) {
        return false;
      }

      const title =
        typeof options.title === 'string' && options.title.trim()
          ? options.title.trim()
          : null;
      if (title) {
        upsertTab(normalizedPath, title);
      } else {
        upsertTab(normalizedPath);
      }

      return activateTab(normalizedPath, {
        pushHistory: options.pushHistory !== false,
        forceReload: !!options.forceReload,
      });
    },
    closeTab(path) {
      if (!state.enabled) {
        return false;
      }
      return closeTab(path);
    },
    getActivePath() {
      return state.activePath;
    },
    isEnabled() {
      return state.enabled;
    },
  };

  window.AdminWorkspace = api;

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', initWorkspace);
  } else {
    initWorkspace();
  }
})();
