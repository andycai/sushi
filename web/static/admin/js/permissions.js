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

  window.permissionsPage = function permissionsPage() {
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
        containerSelector: '#permissions-table-body',
        storageKey: 'admin.permissions.table.v1',
      }),
      editor: drawerFactory(() => ({
        id: null,
        slug: '',
        name: '',
        module: '',
        description: '',
        isSystem: false,
        mode: 'create',
      })),
      editorForm: formFactory(() => ({})),
      deleteModal: modalFactory(() => ({
        id: null,
        slug: '',
        isSystem: false,
      })),
      init() {},
      openCreate() {
        this.deleteModal.hide();
        this.editor.show({
          id: null,
          slug: '',
          name: '',
          module: '',
          description: '',
          isSystem: false,
          mode: 'create',
        });
      },
      openEdit(dataset) {
        this.deleteModal.hide();
        this.editor.show({
          id: Number(dataset?.permissionId || 0) || null,
          slug: dataset?.permissionSlug || '',
          name: dataset?.permissionName || '',
          module: dataset?.permissionModule || '',
          description: dataset?.permissionDescription || '',
          isSystem: parseDatasetBool(dataset?.permissionSystem),
          mode: 'edit',
        });
      },
      closeEditor() {
        this.editor.hide();
      },
      openDelete(dataset) {
        const isSystem = parseDatasetBool(dataset?.permissionSystem);
        if (isSystem) {
          return;
        }

        this.editor.hide();
        this.deleteModal.show({
          id: Number(dataset?.permissionId || 0) || null,
          slug: dataset?.permissionSlug || '',
          isSystem,
        });
      },
      closeDelete() {
        this.deleteModal.hide();
      },
      applySearch() {
        this.table.page = 1;
        this.table.apply('#permissions-table-body');
      },
      onPermissionsTableSwap() {
        this.table.onAfterSwap('#permissions-table-body');
      },
      setPageSize() {
        this.table.setPageSize(this.table.pageSize, '#permissions-table-body');
      },
      setSortMode() {
        this.table.setSortMode(this.table.sortMode, '#permissions-table-body');
      },
      prevPage() {
        this.table.prevPage('#permissions-table-body');
      },
      nextPage() {
        this.table.nextPage('#permissions-table-body');
      },
      notifyFeedback(selector, fallbackLevel) {
        if (window.AdminUI && typeof window.AdminUI.consumeFeedback === 'function') {
          const consumed = window.AdminUI.consumeFeedback(selector, fallbackLevel);
          if (!consumed && fallbackLevel === 'error') {
            window.AdminUI.notify({
              tone: 'danger',
              title: 'Request failed',
              message: 'Unable to complete the permission operation.',
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
            url: '/admin/partials/permissions/table',
            target: '#permissions-table-body',
            onAfterSwap: () => this.onPermissionsTableSwap(),
            errorMessage: 'Unable to refresh the permission list.',
          });
          return;
        }

        fetch('/admin/partials/permissions/table')
          .then((response) => {
            if (!response.ok) {
              throw new Error(`Failed to refresh permissions (${response.status})`);
            }
            return response.text();
          })
          .then((html) => {
            const target = document.querySelector('#permissions-table-body');
            if (!target) {
              return;
            }
            target.innerHTML = html;
            this.onPermissionsTableSwap();
          })
          .catch(() => {
            if (window.AdminUI) {
              window.AdminUI.notify({
                tone: 'danger',
                title: 'Refresh failed',
                message: 'Unable to refresh the permission list.',
              });
            }
          });
      },
      onEditorBeforeRequest() {
        this.editor.busy = true;
      },
      onEditorAfterRequest(event) {
        this.editor.busy = false;
        const successful = this.isSuccessfulRequest(event, '#permissions-feedback');
        this.notifyFeedback('#permissions-feedback', successful ? 'success' : 'error');
        if (successful) {
          this.closeEditor();
          if (!this.responseHasTrigger(event, 'permissions:refresh')) {
            this.refreshTable();
          }
        }
      },
      onDeleteBeforeRequest() {
        this.deleteModal.busy = true;
      },
      onDeleteAfterRequest(event) {
        this.deleteModal.busy = false;
        const successful = this.isSuccessfulRequest(event, '#permissions-feedback');
        this.notifyFeedback('#permissions-feedback', successful ? 'success' : 'error');
        if (successful) {
          this.closeDelete();
          if (!this.responseHasTrigger(event, 'permissions:refresh')) {
            this.refreshTable();
          }
        }
      },
    };
  };
})();
