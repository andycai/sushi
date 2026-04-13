(() => {
  function fallbackModal(factory) {
    const createPayload =
      typeof factory === 'function' ? factory : () => ({});

    return {
      open: false,
      busy: false,
      payload: createPayload(),
      show(payload = {}) {
        this.open = true;
        this.busy = false;
        this.payload = {
          ...createPayload(),
          ...payload,
        };
      },
      hide() {
        this.open = false;
        this.busy = false;
        this.payload = createPayload();
      },
    };
  }

  function fallbackForm(factory) {
    const createValues =
      typeof factory === 'function' ? factory : () => ({});

    return {
      busy: false,
      values: createValues(),
      reset(values = {}) {
        this.busy = false;
        this.values = {
          ...createValues(),
          ...values,
        };
      },
    };
  }

  function fallbackDataTable() {
    return {
      query: '',
      sortMode: 'default',
      page: 1,
      pageSize: 10,
      pageSizeOptions: [10, 20, 50],
      meta: {},
      totalRows: 0,
      filteredRows: 0,
      visibleRows: 0,
      totalPages: 1,
      emptyFiltered: false,
      apply() {},
      onAfterSwap() {},
      reset() {
        this.query = '';
      },
      setPageSize() {},
      setSortMode() {},
      prevPage() {},
      nextPage() {},
    };
  }

  function parseDatasetBool(value) {
    return String(value || '').toLowerCase() === 'true';
  }

  window.menusPage = function menusPage() {
    const modalFactory =
      window.AdminUI && typeof window.AdminUI.createModal === 'function'
        ? window.AdminUI.createModal
        : fallbackModal;
    const drawerFactory =
      window.AdminUI && typeof window.AdminUI.createDrawer === 'function'
        ? window.AdminUI.createDrawer
        : fallbackModal;
    const formFactory =
      window.AdminUI && typeof window.AdminUI.createForm === 'function'
        ? window.AdminUI.createForm
        : fallbackForm;
    const tableFactory =
      window.AdminUI && typeof window.AdminUI.createDataTable === 'function'
        ? window.AdminUI.createDataTable
        : fallbackDataTable;

    return {
      table: tableFactory({
        containerSelector: '#menus-table-body',
        storageKey: 'admin.menus.table.v1',
      }),
      editor: drawerFactory(() => ({
        id: null,
        label: '',
        icon: '',
        route: '',
        position: 0,
        parent_id: '',
        is_hidden: false,
        is_system: false,
        mode: 'create',
      })),
      editorForm: formFactory(() => ({})),
      deleteModal: modalFactory(() => ({
        id: null,
        label: '',
        is_system: false,
      })),
      parentOptions: [],
      parentExcludeId: null,
      _menuCache: [],
      init() {
        this.refreshParentOptions();
      },
      setParentOptions(excludeId = null) {
        const topLevel = this._menuCache
          .filter((item) => !item.parent_id)
          .filter((item) => Number(item.id) !== Number(excludeId || 0))
          .sort((left, right) => {
            const pos = Number(left.position || 0) - Number(right.position || 0);
            if (pos !== 0) {
              return pos;
            }
            return Number(left.id || 0) - Number(right.id || 0);
          })
          .map((item) => ({
            id: Number(item.id || 0),
            label: item.label || `Menu #${item.id}`,
          }));

        this.parentOptions = topLevel;
      },
      refreshParentOptions(excludeId = null) {
        const targetExclude = excludeId === undefined ? this.parentExcludeId : excludeId;
        this.parentExcludeId = targetExclude || null;

        fetch('/admin/api/menu')
          .then((response) => {
            if (!response.ok) {
              throw new Error(`Failed to load menu options (${response.status})`);
            }
            return response.json();
          })
          .then((payload) => {
            this._menuCache = Array.isArray(payload?.menu) ? payload.menu : [];
            this.setParentOptions(this.parentExcludeId);
          })
          .catch(() => {
            this.parentOptions = [];
          });
      },
      openCreate() {
        this.deleteModal.hide();
        this.parentExcludeId = null;
        this.refreshParentOptions(null);
        this.editor.show({
          id: null,
          label: '',
          icon: '',
          route: '',
          position: 0,
          parent_id: '',
          is_hidden: false,
          is_system: false,
          mode: 'create',
        });
      },
      openEdit(dataset) {
        this.deleteModal.hide();
        const id = Number(dataset?.menuId || 0) || null;
        this.parentExcludeId = id;
        this.refreshParentOptions(id);
        this.editor.show({
          id,
          label: dataset?.menuLabel || '',
          icon: dataset?.menuIcon || '',
          route: dataset?.menuRoute || '',
          position: Number(dataset?.menuPosition || 0),
          parent_id: dataset?.menuParentId || '',
          is_hidden: parseDatasetBool(dataset?.menuHidden),
          is_system: parseDatasetBool(dataset?.menuSystem),
          mode: 'edit',
        });
      },
      closeEditor() {
        this.editor.hide();
      },
      openDelete(dataset) {
        const isSystem = parseDatasetBool(dataset?.menuSystem);
        if (isSystem) {
          return;
        }

        this.editor.hide();
        this.deleteModal.show({
          id: Number(dataset?.menuId || 0) || null,
          label: dataset?.menuLabel || '',
          is_system: isSystem,
        });
      },
      closeDelete() {
        this.deleteModal.hide();
      },
      applySearch() {
        this.table.page = 1;
        this.table.apply('#menus-table-body');
      },
      onMenusTableSwap() {
        this.table.onAfterSwap('#menus-table-body');
      },
      setPageSize() {
        this.table.setPageSize(this.table.pageSize, '#menus-table-body');
      },
      setSortMode() {
        this.table.setSortMode(this.table.sortMode, '#menus-table-body');
      },
      prevPage() {
        this.table.prevPage('#menus-table-body');
      },
      nextPage() {
        this.table.nextPage('#menus-table-body');
      },
      notifyFeedback(selector, fallbackLevel) {
        if (window.AdminUI && typeof window.AdminUI.consumeFeedback === 'function') {
          const consumed = window.AdminUI.consumeFeedback(selector, fallbackLevel);
          if (!consumed && fallbackLevel === 'error') {
            window.AdminUI.notify({
              tone: 'danger',
              title: 'Request failed',
              message: 'Unable to complete the menu operation.',
            });
          }
        }
      },
      isErrorFeedback(selector) {
        if (window.AdminUI && typeof window.AdminUI.isErrorFeedback === 'function') {
          return window.AdminUI.isErrorFeedback(selector, 'error');
        }

        const container = document.querySelector(selector);
        if (!container) {
          return false;
        }

        const flash = container.querySelector('[data-ui-flash]');
        if (!flash) {
          return false;
        }

        const level = String(flash.dataset.level || '').toLowerCase();
        return level === 'error' || level === 'danger';
      },
      responseHasTrigger(event, triggerName) {
        if (window.AdminUI && typeof window.AdminUI.hasHxTrigger === 'function') {
          return window.AdminUI.hasHxTrigger(event, triggerName);
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
      },
      isSuccessfulRequest(event, selector) {
        if (!event?.detail?.successful) {
          return false;
        }
        return !this.isErrorFeedback(selector);
      },
      refreshTable() {
        if (window.AdminUI && typeof window.AdminUI.refreshPartial === 'function') {
          window.AdminUI.refreshPartial({
            url: '/admin/partials/menus/table',
            target: '#menus-table-body',
            onAfterSwap: () => {
              this.onMenusTableSwap();
              this.refreshParentOptions();
            },
            errorMessage: 'Unable to refresh the menu list.',
          });
          return;
        }

        fetch('/admin/partials/menus/table')
          .then((response) => {
            if (!response.ok) {
              throw new Error(`Failed to refresh menus (${response.status})`);
            }
            return response.text();
          })
          .then((html) => {
            const target = document.querySelector('#menus-table-body');
            if (!target) {
              return;
            }
            target.innerHTML = html;
            this.onMenusTableSwap();
            this.refreshParentOptions();
          })
          .catch(() => {
            if (window.AdminUI) {
              window.AdminUI.notify({
                tone: 'danger',
                title: 'Refresh failed',
                message: 'Unable to refresh the menu list.',
              });
            }
          });
      },
      onEditorBeforeRequest() {
        this.editor.busy = true;
      },
      onEditorAfterRequest(event) {
        this.editor.busy = false;
        const successful = this.isSuccessfulRequest(event, '#menus-feedback');
        this.notifyFeedback('#menus-feedback', successful ? 'success' : 'error');
        if (successful) {
          this.closeEditor();
          if (!this.responseHasTrigger(event, 'menus:refresh')) {
            this.refreshTable();
          } else {
            this.refreshParentOptions();
          }
        }
      },
      onDeleteBeforeRequest() {
        this.deleteModal.busy = true;
      },
      onDeleteAfterRequest(event) {
        this.deleteModal.busy = false;
        const successful = this.isSuccessfulRequest(event, '#menus-feedback');
        this.notifyFeedback('#menus-feedback', successful ? 'success' : 'error');
        if (successful) {
          this.closeDelete();
          if (!this.responseHasTrigger(event, 'menus:refresh')) {
            this.refreshTable();
          } else {
            this.refreshParentOptions();
          }
        }
      },
    };
  };
})();
