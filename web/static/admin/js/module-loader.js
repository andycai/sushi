(() => {
  const state = {
    moduleScripts: {},
    loaded: new Set(),
    loading: new Map(),
  };

  function normalizeModule(raw) {
    if (!raw || typeof raw !== 'string') {
      return '';
    }
    return raw.trim().replace(/^\/+|\/+$/g, '').toLowerCase();
  }

  function normalizePath(rawPath) {
    if (!rawPath || typeof rawPath !== 'string') {
      return '';
    }

    let url;
    try {
      url = new URL(rawPath, window.location.origin);
    } catch (_) {
      return '';
    }

    let path = url.pathname || '';
    if (path === '/admin') {
      path = '/admin/';
    }
    if (!path.startsWith('/admin/')) {
      return '';
    }
    if (path !== '/admin/') {
      path = path.replace(/\/+$/, '');
    }
    return path;
  }

  function moduleFromPath(path) {
    const normalizedPath = normalizePath(path);
    if (!normalizedPath) {
      return '';
    }
    if (normalizedPath === '/admin/') {
      return 'dashboard';
    }
    return normalizedPath
      .replace(/^\/admin\//, '')
      .split('/')
      .filter(Boolean)
      .map((part) => encodeURIComponent(part))
      .join('/');
  }

  function registerFromDom() {
    const nodes = document.querySelectorAll(
      'script[data-admin-module][data-admin-module-loaded="true"]',
    );
    nodes.forEach((node) => {
      const key = normalizeModule(node.dataset.adminModule || '');
      if (key) {
        state.loaded.add(key);
      }
    });
  }

  function registerModules(moduleMap) {
    if (!moduleMap || typeof moduleMap !== 'object') {
      return;
    }
    Object.entries(moduleMap).forEach(([moduleName, src]) => {
      const key = normalizeModule(moduleName);
      if (!key || typeof src !== 'string' || !src.trim()) {
        return;
      }
      state.moduleScripts[key] = src;
    });
  }

  function markLoaded(moduleName) {
    const key = normalizeModule(moduleName);
    if (key) {
      state.loaded.add(key);
    }
  }

  function loadModule(moduleName) {
    const key = normalizeModule(moduleName);
    if (!key) {
      return Promise.resolve(false);
    }

    if (state.loaded.has(key)) {
      return Promise.resolve(true);
    }

    const scriptUrl = state.moduleScripts[key];
    if (!scriptUrl) {
      return Promise.resolve(false);
    }

    if (state.loading.has(key)) {
      return state.loading.get(key);
    }

    const existing = document.querySelector(`script[data-admin-module="${key}"]`);
    if (existing) {
      if (existing.dataset.adminModuleLoaded === 'true') {
        state.loaded.add(key);
        return Promise.resolve(true);
      }

      const reusedPromise = new Promise((resolve, reject) => {
        const onLoad = () => {
          existing.dataset.adminModuleLoaded = 'true';
          state.loaded.add(key);
          resolve(true);
        };
        const onError = () => {
          reject(new Error(`failed to load module script: ${key}`));
        };
        existing.addEventListener('load', onLoad, { once: true });
        existing.addEventListener('error', onError, { once: true });
      }).finally(() => state.loading.delete(key));

      state.loading.set(key, reusedPromise);
      return reusedPromise;
    }

    const script = document.createElement('script');
    script.src = scriptUrl;
    script.async = true;
    script.dataset.adminModule = key;

    const promise = new Promise((resolve, reject) => {
      script.addEventListener(
        'load',
        () => {
          script.dataset.adminModuleLoaded = 'true';
          state.loaded.add(key);
          resolve(true);
        },
        { once: true },
      );
      script.addEventListener(
        'error',
        () => reject(new Error(`failed to load module script: ${key}`)),
        { once: true },
      );
    }).finally(() => state.loading.delete(key));

    state.loading.set(key, promise);
    document.body.appendChild(script);
    return promise;
  }

  function loadForPath(path) {
    return loadModule(moduleFromPath(path));
  }

  registerFromDom();

  window.AdminModuleLoader = {
    registerModules,
    markLoaded,
    loadModule,
    loadForPath,
    moduleFromPath,
  };
})();
