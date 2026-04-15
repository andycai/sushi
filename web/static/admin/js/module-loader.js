(() => {
  function normalizeStaticPrefix(raw) {
    if (typeof raw !== 'string') {
      return '/static';
    }
    const value = raw.trim();
    if (!value || value === '/') {
      return '/static';
    }
    const prefix = value.startsWith('/') ? value : `/${value}`;
    return prefix.replace(/\/+$/, '');
  }

  function inferStaticPrefix() {
    const current = document.currentScript;
    const scripts = [];
    if (current && typeof current.src === 'string') {
      scripts.push(current.src);
    }
    scripts.push(...Array.from(document.querySelectorAll('script[src]')).map((node) => node.src));

    for (const src of scripts) {
      if (typeof src !== 'string' || !src) {
        continue;
      }
      let url;
      try {
        url = new URL(src, window.location.origin);
      } catch (_) {
        continue;
      }
      const matched = (url.pathname || '').match(/^(.*)\/admin\/js\/module-loader\.js$/);
      if (matched && matched[1]) {
        return normalizeStaticPrefix(matched[1]);
      }
    }

    return '/static';
  }

  function normalizeModuleSegment(raw) {
    if (typeof raw !== 'string') {
      return '';
    }
    const value = raw.trim().toLowerCase();
    if (!value) {
      return '';
    }
    return /^[a-z0-9][a-z0-9_-]*$/.test(value) ? value : '';
  }

  const state = {
    moduleScripts: {},
    loadedModules: new Set(),
    loadingModules: new Map(),
    loadedAssets: new Set(),
    loadingAssets: new Map(),
    pathAssetCache: new Map(),
    staticPrefix: inferStaticPrefix(),
  };

  function normalizeModule(raw) {
    if (!raw || typeof raw !== 'string') {
      return '';
    }
    const segments = raw
      .trim()
      .replace(/^\/+|\/+$/g, '')
      .split('/')
      .filter(Boolean);
    if (!segments.length) {
      return '';
    }

    const normalized = [];
    for (const segment of segments) {
      const value = normalizeModuleSegment(segment);
      if (!value) {
        return '';
      }
      normalized.push(value);
    }
    return normalized.join('/');
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
    return normalizeModule(normalizedPath.replace(/^\/admin\//, ''));
  }

  function moduleCandidates(moduleName) {
    const key = normalizeModule(moduleName);
    if (!key) {
      return [];
    }

    const topLevel = key.split('/')[0];
    if (topLevel && topLevel !== key) {
      return [key, topLevel];
    }
    return [key];
  }

  function resolveModuleScriptUrl(moduleName) {
    const key = normalizeModule(moduleName);
    if (!key) {
      return '';
    }

    const registered = state.moduleScripts[key];
    if (registered) {
      return registered;
    }

    // Keep fallback convention simple and predictable for built-in modules.
    if (key.includes('/')) {
      return '';
    }

    return `${state.staticPrefix}/admin/js/${key}.js`;
  }

  function loadModuleScript(moduleKey, scriptUrl) {
    if (state.loadedModules.has(moduleKey)) {
      return Promise.resolve(true);
    }

    if (!scriptUrl) {
      return Promise.resolve(false);
    }

    if (state.loadingModules.has(moduleKey)) {
      return state.loadingModules.get(moduleKey);
    }

    const existing = document.querySelector(`script[data-admin-module="${moduleKey}"]`);
    if (existing) {
      if (existing.dataset.adminModuleLoaded === 'true') {
        state.loadedModules.add(moduleKey);
        return Promise.resolve(true);
      }

      const reusedPromise = new Promise((resolve, reject) => {
        const onLoad = () => {
          existing.dataset.adminModuleLoaded = 'true';
          state.loadedModules.add(moduleKey);
          resolve(true);
        };
        const onError = () => {
          reject(new Error(`failed to load module script: ${moduleKey}`));
        };
        existing.addEventListener('load', onLoad, { once: true });
        existing.addEventListener('error', onError, { once: true });
      }).finally(() => state.loadingModules.delete(moduleKey));

      state.loadingModules.set(moduleKey, reusedPromise);
      return reusedPromise;
    }

    const script = document.createElement('script');
    script.src = scriptUrl;
    script.async = true;
    script.dataset.adminModule = moduleKey;

    const promise = new Promise((resolve, reject) => {
      script.addEventListener(
        'load',
        () => {
          script.dataset.adminModuleLoaded = 'true';
          state.loadedModules.add(moduleKey);
          resolve(true);
        },
        { once: true },
      );
      script.addEventListener(
        'error',
        () => reject(new Error(`failed to load module script: ${moduleKey}`)),
        { once: true },
      );
    }).finally(() => state.loadingModules.delete(moduleKey));

    state.loadingModules.set(moduleKey, promise);
    document.body.appendChild(script);
    return promise;
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
      const normalizedSrc = normalizeAssetUrl(src, 'js');
      if (!key || !normalizedSrc) {
        return;
      }
      state.moduleScripts[key] = normalizedSrc;
    });
  }

  function markLoaded(moduleName) {
    const key = normalizeModule(moduleName);
    if (key) {
      state.loadedModules.add(key);
    }
  }

  function loadModule(moduleName) {
    const candidates = moduleCandidates(moduleName);
    if (!candidates.length) {
      return Promise.resolve(false);
    }

    const loadNext = (index) => {
      if (index >= candidates.length) {
        return Promise.resolve(false);
      }

      const key = candidates[index];
      const scriptUrl = resolveModuleScriptUrl(key);
      if (!scriptUrl) {
        return loadNext(index + 1);
      }

      return loadModuleScript(key, scriptUrl).catch((err) => {
        if (index + 1 < candidates.length) {
          return loadNext(index + 1);
        }
        throw err;
      });
    };

    return loadNext(0);
  }

  function normalizeAssetUrl(value, kind) {
    if (typeof value !== 'string') {
      return '';
    }

    const raw = value.trim();
    if (!raw) {
      return '';
    }

    let url;
    try {
      url = new URL(raw, window.location.origin);
    } catch (_) {
      return '';
    }

    const pathname = (url.pathname || '').toLowerCase();
    if (!pathname) {
      return '';
    }

    if (kind === 'js' && !pathname.endsWith('.js')) {
      return '';
    }
    if (kind === 'css' && !pathname.endsWith('.css')) {
      return '';
    }

    return `${url.pathname}${url.search}${url.hash}`;
  }

  function sanitizeAssetList(value, kind) {
    if (!Array.isArray(value)) {
      return [];
    }
    return value
      .map((item) => normalizeAssetUrl(item, kind))
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
          js: sanitizeAssetList(payload && payload.js, 'js'),
          css: sanitizeAssetList(payload && payload.css, 'css'),
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
