(() => {
  const state = {
    moduleScripts: {},
    loadedModules: new Set(),
    loadingModules: new Map(),
    loadedAssets: new Set(),
    loadingAssets: new Map(),
    pathAssetCache: new Map(),
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
        state.loadedModules.add(key);
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
      state.loadedModules.add(key);
    }
  }

  function loadModule(moduleName) {
    const key = normalizeModule(moduleName);
    if (!key) {
      return Promise.resolve(false);
    }

    if (state.loadedModules.has(key)) {
      return Promise.resolve(true);
    }

    const scriptUrl = state.moduleScripts[key];
    if (!scriptUrl) {
      return Promise.resolve(false);
    }

    if (state.loadingModules.has(key)) {
      return state.loadingModules.get(key);
    }

    const existing = document.querySelector(`script[data-admin-module="${key}"]`);
    if (existing) {
      if (existing.dataset.adminModuleLoaded === 'true') {
        state.loadedModules.add(key);
        return Promise.resolve(true);
      }

      const reusedPromise = new Promise((resolve, reject) => {
        const onLoad = () => {
          existing.dataset.adminModuleLoaded = 'true';
          state.loadedModules.add(key);
          resolve(true);
        };
        const onError = () => {
          reject(new Error(`failed to load module script: ${key}`));
        };
        existing.addEventListener('load', onLoad, { once: true });
        existing.addEventListener('error', onError, { once: true });
      }).finally(() => state.loadingModules.delete(key));

      state.loadingModules.set(key, reusedPromise);
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
          state.loadedModules.add(key);
          resolve(true);
        },
        { once: true },
      );
      script.addEventListener(
        'error',
        () => reject(new Error(`failed to load module script: ${key}`)),
        { once: true },
      );
    }).finally(() => state.loadingModules.delete(key));

    state.loadingModules.set(key, promise);
    document.body.appendChild(script);
    return promise;
  }

  function sanitizeAssetList(value) {
    if (!Array.isArray(value)) {
      return [];
    }
    return value
      .map((item) => (typeof item === 'string' ? item.trim() : ''))
      .filter(Boolean);
  }

  function assetKey(kind, assetUrl) {
    return `${kind}:${assetUrl}`;
  }

  function fetchAssetsForPath(path) {
    const normalizedPath = normalizePath(path);
    if (!normalizedPath) {
      return Promise.resolve({ js: [], css: [] });
    }

    if (state.pathAssetCache.has(normalizedPath)) {
      return Promise.resolve(state.pathAssetCache.get(normalizedPath));
    }

    return fetch(
      `/admin/api/workspace/assets?path=${encodeURIComponent(normalizedPath)}`,
      {
        headers: {
          Accept: 'application/json',
        },
      },
    )
      .then((resp) => {
        if (!resp.ok) {
          return { js: [], css: [] };
        }
        return resp.json().catch(() => ({ js: [], css: [] }));
      })
      .then((payload) => {
        const normalized = {
          js: sanitizeAssetList(payload && payload.js),
          css: sanitizeAssetList(payload && payload.css),
        };
        state.pathAssetCache.set(normalizedPath, normalized);
        return normalized;
      })
      .catch(() => ({ js: [], css: [] }));
  }

  function loadCss(assetUrl) {
    const key = assetKey('css', assetUrl);
    if (state.loadedAssets.has(key)) {
      return Promise.resolve(true);
    }
    if (state.loadingAssets.has(key)) {
      return state.loadingAssets.get(key);
    }

    const existing = document.querySelector(`link[data-admin-asset-css="${assetUrl}"]`);
    if (existing) {
      state.loadedAssets.add(key);
      return Promise.resolve(true);
    }

    const link = document.createElement('link');
    link.rel = 'stylesheet';
    link.href = assetUrl;
    link.dataset.adminAssetCss = assetUrl;

    const promise = new Promise((resolve, reject) => {
      link.addEventListener(
        'load',
        () => {
          state.loadedAssets.add(key);
          resolve(true);
        },
        { once: true },
      );
      link.addEventListener(
        'error',
        () => reject(new Error(`failed to load css asset: ${assetUrl}`)),
        { once: true },
      );
    }).finally(() => state.loadingAssets.delete(key));

    state.loadingAssets.set(key, promise);
    document.head.appendChild(link);
    return promise;
  }

  function loadJs(assetUrl) {
    const key = assetKey('js', assetUrl);
    if (state.loadedAssets.has(key)) {
      return Promise.resolve(true);
    }
    if (state.loadingAssets.has(key)) {
      return state.loadingAssets.get(key);
    }

    const existing = document.querySelector(`script[data-admin-asset-js="${assetUrl}"]`);
    if (existing) {
      if (existing.dataset.adminAssetLoaded === 'true') {
        state.loadedAssets.add(key);
        return Promise.resolve(true);
      }

      const reusedPromise = new Promise((resolve, reject) => {
        const onLoad = () => {
          existing.dataset.adminAssetLoaded = 'true';
          state.loadedAssets.add(key);
          resolve(true);
        };
        const onError = () => reject(new Error(`failed to load js asset: ${assetUrl}`));
        existing.addEventListener('load', onLoad, { once: true });
        existing.addEventListener('error', onError, { once: true });
      }).finally(() => state.loadingAssets.delete(key));

      state.loadingAssets.set(key, reusedPromise);
      return reusedPromise;
    }

    const script = document.createElement('script');
    script.src = assetUrl;
    script.async = false;
    script.dataset.adminAssetJs = assetUrl;

    const promise = new Promise((resolve, reject) => {
      script.addEventListener(
        'load',
        () => {
          script.dataset.adminAssetLoaded = 'true';
          state.loadedAssets.add(key);
          resolve(true);
        },
        { once: true },
      );
      script.addEventListener(
        'error',
        () => reject(new Error(`failed to load js asset: ${assetUrl}`)),
        { once: true },
      );
    }).finally(() => state.loadingAssets.delete(key));

    state.loadingAssets.set(key, promise);
    document.body.appendChild(script);
    return promise;
  }

  async function loadAssetsForPath(path) {
    const assets = await fetchAssetsForPath(path);

    for (const cssUrl of assets.css) {
      await loadCss(cssUrl);
    }

    for (const jsUrl of assets.js) {
      await loadJs(jsUrl);
    }

    return assets;
  }

  function loadForPath(path) {
    return Promise.resolve()
      .then(() => loadModule(moduleFromPath(path)))
      .then(() => loadAssetsForPath(path));
  }

  registerFromDom();

  window.AdminModuleLoader = {
    registerModules,
    markLoaded,
    loadModule,
    loadAssetsForPath,
    loadForPath,
    moduleFromPath,
  };
})();
