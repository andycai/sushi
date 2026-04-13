(() => {
  window.adminMenu = function adminMenu() {
    return {
      menuItems: [],
      expandedMenus: {},
      activePath: '/admin/',

      async init() {
        this.refreshActivePath();
        await this.loadMenu();

        window.addEventListener('admin:workspace:change', (event) => {
          const nextPath = this.normalizeRoute(event?.detail?.path);
          if (!nextPath) {
            return;
          }
          this.activePath = nextPath;
          this.expandForRoute(nextPath);
        });
      },

      async loadMenu() {
        try {
          const resp = await fetch('/admin/api/menu');
          if (resp.ok) {
            const data = await resp.json();
            this.menuItems = data.menu || [];
            this.expandForRoute(this.activePath);
          }
        } catch (e) {
          console.error('Failed to load menu:', e);
        }
      },

      normalizeRoute(route) {
        if (!route || typeof route !== 'string') {
          return '';
        }
        let path = route.trim();
        if (!path) {
          return '';
        }
        if (path === '/admin') {
          return '/admin/';
        }
        if (path !== '/admin/' && path.endsWith('/')) {
          path = path.replace(/\/+$/, '');
        }
        return path;
      },

      refreshActivePath() {
        const workspacePath =
          window.AdminWorkspace &&
          typeof window.AdminWorkspace.getActivePath === 'function'
            ? window.AdminWorkspace.getActivePath()
            : '';
        this.activePath =
          this.normalizeRoute(workspacePath) ||
          this.normalizeRoute(window.location.pathname) ||
          '/admin/';
      },

      expandForRoute(route) {
        const normalizedRoute = this.normalizeRoute(route);
        if (!normalizedRoute || !this.menuItems.length) {
          return;
        }

        const byId = new Map(this.menuItems.map((item) => [item.id, item]));
        const target = this.menuItems.find(
          (item) =>
            !item.is_hidden && this.normalizeRoute(item.route || '') === normalizedRoute,
        );
        if (!target) {
          return;
        }

        let changed = false;
        let current = target;
        while (current && current.parent_id) {
          if (!this.expandedMenus[current.parent_id]) {
            this.expandedMenus[current.parent_id] = true;
            changed = true;
          }
          current = byId.get(current.parent_id) || null;
        }

        if (changed) {
          this.expandedMenus = { ...this.expandedMenus };
        }
      },

      sortMenuItems(items) {
        return [...items].sort((left, right) => {
          const positionDiff = Number(left?.position || 0) - Number(right?.position || 0);
          if (positionDiff !== 0) {
            return positionDiff;
          }
          return Number(left?.id || 0) - Number(right?.id || 0);
        });
      },

      topMenuItems() {
        return this.sortMenuItems(
          this.menuItems.filter((item) => !item.parent_id && !item.is_hidden),
        );
      },

      hasChildren(item) {
        return this.menuItems.some(
          (entry) => entry.parent_id === item.id && !entry.is_hidden,
        );
      },

      getChildren(parentId) {
        return this.sortMenuItems(
          this.menuItems.filter((entry) => entry.parent_id === parentId && !entry.is_hidden),
        );
      },

      isExpanded(itemId) {
        return !!this.expandedMenus[itemId];
      },

      toggleExpand(itemId) {
        this.expandedMenus[itemId] = !this.expandedMenus[itemId];
        // Force Alpine.js reactivity
        this.expandedMenus = { ...this.expandedMenus };
      },

      hasActiveDescendant(itemId, visited = new Set()) {
        if (visited.has(itemId)) {
          return false;
        }
        visited.add(itemId);

        const activeRoute = this.normalizeRoute(this.activePath);
        return this.getChildren(itemId).some((child) => {
          const childRoute = this.normalizeRoute(child.route || '');
          if (childRoute && childRoute === activeRoute) {
            return true;
          }
          return this.hasActiveDescendant(child.id, visited);
        });
      },

      isActive(item) {
        const activeRoute = this.normalizeRoute(this.activePath);
        const ownRoute = this.normalizeRoute(item.route || '');

        if (ownRoute && ownRoute === activeRoute) {
          return true;
        }

        return this.hasActiveDescendant(item.id);
      },

      handleMenuClick(event, item) {
        if (this.hasChildren(item)) {
          if (event && typeof event.preventDefault === 'function') {
            event.preventDefault();
          }
          this.toggleExpand(item.id);
          return;
        }

        const route = this.normalizeRoute(item.route || '');
        if (!route) {
          return;
        }

        if (
          window.AdminWorkspace &&
          typeof window.AdminWorkspace.openTab === 'function' &&
          window.AdminWorkspace.isEnabled &&
          window.AdminWorkspace.isEnabled()
        ) {
          if (event && typeof event.preventDefault === 'function') {
            event.preventDefault();
          }
          const opened = window.AdminWorkspace.openTab(route, { title: item.label });
          if (!opened) {
            window.location.href = route;
            return;
          }
          this.activePath = route;
          this.expandForRoute(route);
        }
      },

      async logout() {
        await fetch('/api/auth/logout', { method: 'POST' });
        window.location.href = '/admin-login';
      },

      getIcon(iconName) {
        const name = String(iconName || '').trim();
        const icons = {
          'layout-dashboard': '<svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect width="7" height="9" x="3" y="3" rx="1"/><rect width="7" height="5" x="14" y="3" rx="1"/><rect width="7" height="9" x="14" y="12" rx="1"/><rect width="7" height="5" x="3" y="16" rx="1"/></svg>',
          'users': '<svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2"/><circle cx="9" cy="7" r="4"/><path d="M22 21v-2a4 4 0 0 0-3-3.87"/><path d="M16 3.13a4 4 0 0 1 0 7.75"/></svg>',
          'shield': '<svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10"/></svg>',
          'key': '<svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="7.5" cy="15.5" r="5.5"/><path d="m21 2-9.6 9.6"/><path d="m15.5 7.5 3 3L22 7l-3-3"/></svg>',
          'package': '<svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m7.5 4.27 9 5.15"/><path d="M21 8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16Z"/><path d="m3.3 7 8.7 5 8.7-5"/><path d="M12 22V12"/></svg>',
          'settings': '<svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"/><circle cx="12" cy="12" r="3"/></svg>',
          'file-text': '<svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14.5 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7.5L14.5 2z"/><polyline points="14 2 14 8 20 8"/><line x1="16" x2="8" y1="13" y2="13"/><line x1="16" x2="8" y1="17" y2="17"/><line x1="10" x2="8" y1="9" y2="9"/></svg>',
          'database': '<svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><ellipse cx="12" cy="5" rx="9" ry="3"/><path d="M3 5V19A9 3 0 0 0 21 19V5"/><path d="M3 12A9 3 0 0 0 21 12"/></svg>',
          'folder': '<svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.4a2 2 0 0 1-1.6-.8l-.8-1A2 2 0 0 0 8.6 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z"/></svg>',
          'dot': '<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="currentColor" stroke="none"><circle cx="12" cy="12" r="4"/></svg>',
          'log-out': '<svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4"/><polyline points="16 17 21 12 16 7"/><line x1="21" x2="9" y1="12" y2="12"/></svg>',
        };
        return icons[name] || icons.dot;
      }
    };
  };
})();
