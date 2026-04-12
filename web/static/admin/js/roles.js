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

  window.rolesPage = function rolesPage() {
    const modalFactory =
      window.AdminUI && typeof window.AdminUI.createModal === 'function'
        ? window.AdminUI.createModal
        : fallbackModal;
    const drawerFactory =
      window.AdminUI && typeof window.AdminUI.createDrawer === 'function'
        ? window.AdminUI.createDrawer
        : fallbackModal;
    const tableFactory =
      window.AdminUI && typeof window.AdminUI.createDataTable === 'function'
        ? window.AdminUI.createDataTable
        : fallbackDataTable;

    return {
      table: tableFactory({
        containerSelector: '#roles-table-body',
        storageKey: 'admin.roles.table.v1',
      }),
      roleEditor: drawerFactory(() => ({
        id: null,
        slug: '',
        name: '',
        description: '',
        isSystem: false,
        mode: 'edit',
      })),
      permissionsModal: modalFactory(() => ({
        id: null,
        name: '',
      })),
      deleteModal: modalFactory(() => ({
        id: null,
        slug: '',
      })),
      init() {},
      openCreate() {
        this.permissionsModal.hide();
        this.deleteModal.hide();
        this.roleEditor.show({
          id: null,
          slug: '',
          name: '',
          description: '',
          isSystem: false,
          mode: 'create',
        });
      },
      openRoleEditor(dataset) {
        this.permissionsModal.hide();
        this.deleteModal.hide();
        this.roleEditor.show({
          id: Number(dataset?.roleId || 0) || null,
          slug: dataset?.roleSlug || '',
          name: dataset?.roleName || '',
          description: dataset?.roleDescription || '',
          isSystem: parseDatasetBool(dataset?.roleSystem),
          mode: 'edit',
        });
      },
      closeRoleEditor() {
        this.roleEditor.hide();
      },
      openRolePermissions(dataset) {
        const roleId = Number(dataset?.roleId || 0);
        if (!roleId) {
          return;
        }

        this.roleEditor.hide();
        this.permissionsModal.show({
          id: roleId,
          name: dataset?.roleName || '',
        });

        const target = '#roles-permissions-form-shell';
        const url = `/admin/partials/roles/${roleId}/permissions/form`;

        if (window.AdminUI && typeof window.AdminUI.refreshPartial === 'function') {
          window.AdminUI.refreshPartial({
            url,
            target,
            errorMessage: 'Unable to load role permissions.',
          });
          return;
        }

        fetch(url)
          .then((response) => {
            if (!response.ok) {
              throw new Error(`Failed to load role permissions (${response.status})`);
            }
            return response.text();
          })
          .then((html) => {
            const targetNode = document.querySelector(target);
            if (targetNode) {
              targetNode.innerHTML = html;
            }
          })
          .catch(() => {
            const targetNode = document.querySelector(target);
            if (targetNode) {
              targetNode.innerHTML =
                '<div class="ui-state-panel danger">Unable to load role permissions.</div>';
            }
          });
      },
      closePermissionsModal() {
        this.permissionsModal.hide();
      },
      openDelete(dataset) {
        const isSystem = parseDatasetBool(dataset?.roleSystem);
        if (isSystem) {
          return;
        }
        this.permissionsModal.hide();
        this.roleEditor.hide();
        this.deleteModal.show({
          id: Number(dataset?.roleId || 0) || null,
          slug: dataset?.roleSlug || '',
        });
      },
      closeDelete() {
        this.deleteModal.hide();
      },
      applySearch() {
        this.table.page = 1;
        this.table.apply('#roles-table-body');
      },
      onRolesTableSwap() {
        this.table.onAfterSwap('#roles-table-body');
      },
      setPageSize() {
        this.table.setPageSize(this.table.pageSize, '#roles-table-body');
      },
      setSortMode() {
        this.table.setSortMode(this.table.sortMode, '#roles-table-body');
      },
      prevPage() {
        this.table.prevPage('#roles-table-body');
      },
      nextPage() {
        this.table.nextPage('#roles-table-body');
      },
      notifyFeedback(selector, fallbackLevel) {
        if (window.AdminUI && typeof window.AdminUI.consumeFeedback === 'function') {
          const consumed = window.AdminUI.consumeFeedback(selector, fallbackLevel);
          if (!consumed && fallbackLevel === 'error') {
            window.AdminUI.notify({
              tone: 'danger',
              title: 'Request failed',
              message: 'Unable to complete the roles operation.',
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
            url: '/admin/partials/roles/table',
            target: '#roles-table-body',
            onAfterSwap: () => this.onRolesTableSwap(),
            errorMessage: 'Unable to refresh the roles list.',
          });
          return;
        }

        fetch('/admin/partials/roles/table')
          .then((response) => {
            if (!response.ok) {
              throw new Error(`Failed to refresh roles (${response.status})`);
            }
            return response.text();
          })
          .then((html) => {
            const target = document.querySelector('#roles-table-body');
            if (!target) {
              return;
            }
            target.innerHTML = html;
            this.onRolesTableSwap();
          })
          .catch(() => {
            if (window.AdminUI) {
              window.AdminUI.notify({
                tone: 'danger',
                title: 'Refresh failed',
                message: 'Unable to refresh the roles list.',
              });
            }
          });
      },
      onRoleEditorBeforeRequest() {
        this.roleEditor.busy = true;
      },
      onRoleEditorAfterRequest(event) {
        this.roleEditor.busy = false;
        const successful = this.isSuccessfulRequest(event, '#roles-feedback');
        this.notifyFeedback('#roles-feedback', successful ? 'success' : 'error');
        if (successful) {
          this.closeRoleEditor();
          if (!this.responseHasTrigger(event, 'roles:refresh')) {
            this.refreshTable();
          }
        }
      },
      onRolePermissionsBeforeRequest() {
        this.permissionsModal.busy = true;
      },
      onRolePermissionsAfterRequest(event) {
        this.permissionsModal.busy = false;
        const successful = this.isSuccessfulRequest(event, '#roles-feedback');
        this.notifyFeedback('#roles-feedback', successful ? 'success' : 'error');
        if (successful) {
          this.closePermissionsModal();
          if (!this.responseHasTrigger(event, 'roles:refresh')) {
            this.refreshTable();
          }
        }
      },
      onDeleteBeforeRequest() {
        this.deleteModal.busy = true;
      },
      onDeleteAfterRequest(event) {
        this.deleteModal.busy = false;
        const successful = this.isSuccessfulRequest(event, '#roles-feedback');
        this.notifyFeedback('#roles-feedback', successful ? 'success' : 'error');
        if (successful) {
          this.closeDelete();
          if (!this.responseHasTrigger(event, 'roles:refresh')) {
            this.refreshTable();
          }
        }
      },
    };
  };
})();
