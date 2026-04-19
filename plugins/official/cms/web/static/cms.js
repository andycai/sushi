(() => {
  const OVERVIEW = 'overview';
  const LIBRARY = 'library';
  const EDITOR = 'editor';
  const DEFAULT_LIBRARY_SCOPE = 'posts';
  const VALID_SCOPES = new Set(['posts', 'pages', 'categories']);

  function toScope(value) {
    const normalized = String(value || '').toLowerCase();
    if (VALID_SCOPES.has(normalized)) {
      return normalized;
    }
    return DEFAULT_LIBRARY_SCOPE;
  }

  function isTypingTarget(target) {
    if (!target) {
      return false;
    }
    const tag = (target.tagName || '').toLowerCase();
    return (
      tag === 'input' ||
      tag === 'textarea' ||
      tag === 'select' ||
      target.isContentEditable
    );
  }

  function closest(el, selector) {
    if (!el || typeof el.closest !== 'function') {
      return null;
    }
    return el.closest(selector);
  }

  window.cmsPage = function cmsPage() {
    return {
      panel: OVERVIEW,
      activeNav: OVERVIEW,
      libraryScope: DEFAULT_LIBRARY_SCOPE,
      commandOpen: false,
      pendingGotoPrefix: false,
      selectedRowIndex: 0,
      libraryRows: [],

      init() {
        this.bindEvents();
      },

      bindEvents() {
        window.addEventListener('keydown', (event) => this.handleGlobalShortcut(event));

        document.body.addEventListener('click', (event) => {
          const openLibraryTrigger = closest(event.target, '[data-cms-open-library]');
          if (openLibraryTrigger) {
            event.preventDefault();
            this.goLibrary(openLibraryTrigger.dataset.scope || DEFAULT_LIBRARY_SCOPE);
            return;
          }

          const openEditorTrigger = closest(event.target, '[data-cms-open-editor]');
          if (openEditorTrigger) {
            event.preventDefault();
            this.openEditor(
              toScope(openEditorTrigger.dataset.resource),
              openEditorTrigger.dataset.slug || 'new',
            );
          }
        });

        document.body.addEventListener('input', (event) => {
          if (!closest(event.target, '[data-cms-library-search]')) {
            return;
          }
          this.filterRows(event.target.value || '');
        });
      },

      switchPanel(next) {
        if (next !== OVERVIEW && next !== LIBRARY && next !== EDITOR) {
          return;
        }
        this.panel = next;
      },

      isPanel(name) {
        return this.panel === name;
      },

      isNavActive(name) {
        return this.activeNav === name;
      },

      goOverview() {
        this.activeNav = OVERVIEW;
        this.switchPanel(OVERVIEW);
        this.dispatchHtmxTrigger('cms:panel:overview');
      },

      goLibrary(scope) {
        const resolvedScope = toScope(scope);
        this.activeNav = resolvedScope;
        this.libraryScope = resolvedScope;
        this.switchPanel(LIBRARY);
        this.loadPanel('#cms-library-panel', `/admin/partials/cms/library/${resolvedScope}`);
      },

      openEditor(resource, slug) {
        const resolvedResource = toScope(resource);
        const resolvedSlug = slug && String(slug).trim() !== '' ? String(slug).trim() : 'new';
        this.activeNav = resolvedResource;
        this.switchPanel(EDITOR);
        this.loadPanel('#cms-editor-panel', `/admin/partials/cms/editor/${resolvedResource}/${encodeURIComponent(resolvedSlug)}`);
      },

      openCommandPalette() {
        this.commandOpen = true;
        this.dispatchHtmxTrigger('cms:commands:refresh');
      },

      closeCommandPalette() {
        this.commandOpen = false;
      },

      loadPanel(targetSelector, url) {
        if (window.htmx && typeof window.htmx.ajax === 'function') {
          window.htmx.ajax('GET', url, {
            target: targetSelector,
            swap: 'innerHTML',
          });
          return;
        }
        const target = document.querySelector(targetSelector);
        if (!target) {
          return;
        }
        fetch(url)
          .then((response) => response.text())
          .then((html) => {
            target.innerHTML = html;
          })
          .catch(() => {
            target.innerHTML = '<div class="ui-empty">Unable to load this panel.</div>';
          });
      },

      dispatchHtmxTrigger(name) {
        if (window.htmx && typeof window.htmx.trigger === 'function') {
          window.htmx.trigger(document.body, name);
        }
      },

      focusLibrarySearch() {
        const input = document.querySelector('[data-cms-library-search]');
        if (input) {
          input.focus();
          input.select();
        }
      },

      collectLibraryRows() {
        const rows = Array.from(document.querySelectorAll('#cms-library-table-body [data-cms-row]'));
        this.libraryRows = rows;
        return rows;
      },

      highlightLibrarySelection() {
        const rows = this.collectLibraryRows();
        rows.forEach((row, index) => {
          row.classList.toggle('is-selected', index === this.selectedRowIndex);
        });
      },

      moveRowSelection(direction) {
        const rows = this.collectLibraryRows();
        if (rows.length === 0) {
          this.selectedRowIndex = 0;
          return;
        }
        if (direction > 0) {
          this.selectedRowIndex = Math.min(rows.length - 1, this.selectedRowIndex + 1);
        } else {
          this.selectedRowIndex = Math.max(0, this.selectedRowIndex - 1);
        }
        this.highlightLibrarySelection();
      },

      openSelectedRow() {
        const rows = this.collectLibraryRows();
        const selected = rows[this.selectedRowIndex];
        if (!selected) {
          return;
        }
        this.openEditor(selected.dataset.resource || this.libraryScope, selected.dataset.slug || 'new');
      },

      deleteSelectedRow() {
        const rows = this.collectLibraryRows();
        const selected = rows[this.selectedRowIndex];
        if (!selected) {
          return;
        }
        const form = selected.querySelector('form[hx-post*="/delete"]');
        if (!form) {
          return;
        }
        if (window.htmx && typeof window.htmx.trigger === 'function') {
          window.htmx.trigger(form, 'submit');
        } else {
          form.requestSubmit();
        }
      },

      saveEditor() {
        const form = document.getElementById('cms-editor-form');
        if (!form) {
          return;
        }
        if (window.htmx && typeof window.htmx.trigger === 'function') {
          window.htmx.trigger(form, 'submit');
        } else {
          form.requestSubmit();
        }
      },

      publishEditor() {
        const transitionForm = document.getElementById('cms-transition-form');
        if (!transitionForm) {
          return;
        }
        const statusInput = transitionForm.querySelector('select[name="next_status"], input[name="next_status"]');
        if (statusInput) {
          statusInput.value = 'published';
        }
        if (window.htmx && typeof window.htmx.trigger === 'function') {
          window.htmx.trigger(transitionForm, 'submit');
        } else {
          transitionForm.requestSubmit();
        }
      },

      openStatusTransition() {
        const transitionForm = document.getElementById('cms-transition-form');
        if (!transitionForm) {
          return;
        }
        const statusInput = transitionForm.querySelector('select[name="next_status"], input[name="next_status"]');
        if (statusInput && typeof statusInput.focus === 'function') {
          statusInput.focus();
        }
      },

      filterRows(query) {
        const value = String(query || '').trim().toLowerCase();
        const rows = this.collectLibraryRows();
        rows.forEach((row) => {
          const text = row.textContent ? row.textContent.toLowerCase() : '';
          row.hidden = value !== '' && !text.includes(value);
        });
        this.selectedRowIndex = 0;
        this.highlightLibrarySelection();
      },

      handleGotoSequence(event) {
        if (event.defaultPrevented) {
          return false;
        }

        const key = String(event.key || '').toLowerCase();
        if (!this.pendingGotoPrefix) {
          if (key === 'g' && !event.metaKey && !event.ctrlKey && !event.altKey) {
            this.pendingGotoPrefix = true;
            window.setTimeout(() => {
              this.pendingGotoPrefix = false;
            }, 900);
            return true;
          }
          return false;
        }

        this.pendingGotoPrefix = false;
        if (key === 'o') {
          this.goOverview();
          return true;
        }
        if (key === 'p') {
          this.goLibrary('posts');
          return true;
        }
        if (key === 'a') {
          this.openEditor('posts', 'new');
          return true;
        }
        return false;
      },

      handleGlobalShortcut(event) {
        const cmd = event.metaKey || event.ctrlKey;
        const key = String(event.key || '').toLowerCase();

        if (key === 'escape' && this.commandOpen) {
          event.preventDefault();
          this.closeCommandPalette();
          return;
        }

        if (cmd && key === 'k') {
          // Cmd/Ctrl+K opens command palette.
          event.preventDefault();
          this.openCommandPalette();
          return;
        }

        if (cmd && key === 's') {
          event.preventDefault();
          this.saveEditor();
          return;
        }

        if (cmd && key === 'enter') {
          event.preventDefault();
          this.publishEditor();
          return;
        }

        if (cmd && event.shiftKey && key === 'p') {
          event.preventDefault();
          this.openStatusTransition();
          return;
        }

        if (this.handleGotoSequence(event)) {
          event.preventDefault();
          return;
        }

        if (isTypingTarget(event.target) && key !== '/') {
          return;
        }

        if (this.panel === LIBRARY && key === '/') {
          event.preventDefault();
          this.focusLibrarySearch();
          return;
        }

        if (this.panel === LIBRARY && key === 'j') {
          event.preventDefault();
          this.moveRowSelection(1);
          return;
        }

        if (this.panel === LIBRARY && key === 'k' && !cmd) {
          event.preventDefault();
          this.moveRowSelection(-1);
          return;
        }

        if (this.panel === LIBRARY && key === 'e') {
          event.preventDefault();
          this.openSelectedRow();
          return;
        }

        if (this.panel === LIBRARY && (key === 'delete' || key === 'backspace')) {
          event.preventDefault();
          this.deleteSelectedRow();
        }
      },
    };
  };
})();
