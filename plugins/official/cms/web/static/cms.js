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

  function escapeHtml(value) {
    return String(value || '')
      .replaceAll('&', '&amp;')
      .replaceAll('<', '&lt;')
      .replaceAll('>', '&gt;')
      .replaceAll('"', '&quot;')
      .replaceAll("'", '&#39;');
  }

  function markdownToHtml(markdown) {
    const escaped = escapeHtml(markdown).replaceAll('\r\n', '\n');
    return `<p>${escaped.replace(/\n\n+/g, '</p><p>')}</p>`;
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
      pendingLibraryHighlightSlug: '',

      init() {
        this.bindEvents();
      },

      bindEvents() {
        window.addEventListener('keydown', (event) => this.handleGlobalShortcut(event));
        this.initMarkdownEditors(document);
        this.ensureToastStack();

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

        document.body.addEventListener('htmx:afterSwap', (event) => {
          const target = event && event.target;
          if (!target || !target.id) {
            return;
          }

          if (target.id === 'cms-editor-panel') {
            this.initMarkdownEditors(target);
            return;
          }

          if (target.id === 'cms-library-panel') {
            this.selectedRowIndex = 0;
            this.highlightLibrarySelection();
            this.applyPendingLibraryHighlight();
          }
        });

        document.body.addEventListener('htmx:afterRequest', (event) => {
          this.handleFeedbackResponse(event);
        });
      },

      initMarkdownEditors(root) {
        const container = root && typeof root.querySelectorAll === 'function' ? root : document;
        const helpers = container.querySelectorAll('[data-cms-markdown-helper]');
        helpers.forEach((helper) => {
          if (helper.dataset.cmsMdBound === 'true') {
            return;
          }
          const input = helper.querySelector('[data-cms-markdown-input]');
          const preview = helper.querySelector('[data-cms-markdown-preview]');
          if (!input || !preview) {
            return;
          }

          helper.dataset.cmsMdBound = 'true';
          this.setMarkdownMode(helper, 'write');
          this.renderMarkdownPreview(input, preview);

          helper.addEventListener('click', (event) => {
            const trigger = closest(event.target, '[data-cms-md-action]');
            if (!trigger) {
              return;
            }
            event.preventDefault();
            this.applyMarkdownAction(
              trigger.dataset.cmsMdAction || '',
              input,
              helper,
              preview,
            );
          });

          input.addEventListener('input', () => {
            this.renderMarkdownPreview(input, preview);
          });
        });
      },

      setMarkdownMode(helper, mode) {
        const input = helper.querySelector('[data-cms-markdown-input]');
        const preview = helper.querySelector('[data-cms-markdown-preview]');
        const isPreview = mode === 'preview';
        if (input) {
          input.hidden = isPreview;
        }
        if (preview) {
          preview.hidden = !isPreview;
        }
        const toggles = helper.querySelectorAll('[data-cms-md-action="write"], [data-cms-md-action="preview"]');
        toggles.forEach((toggle) => {
          toggle.classList.toggle('is-active', toggle.dataset.cmsMdAction === mode);
        });
      },

      renderMarkdownPreview(input, preview) {
        if (!input || !preview) {
          return;
        }
        preview.innerHTML = markdownToHtml(input.value || '');
      },

      surroundSelection(input, prefix, suffix, placeholder) {
        const start = input.selectionStart || 0;
        const end = input.selectionEnd || 0;
        const current = input.value.slice(start, end);
        const content = current !== '' ? current : placeholder;
        input.setRangeText(`${prefix}${content}${suffix}`, start, end, 'end');
        input.focus();
      },

      prefixSelectionLines(input, prefix) {
        const start = input.selectionStart || 0;
        const end = input.selectionEnd || 0;
        const lineStart = input.value.lastIndexOf('\n', Math.max(0, start - 1)) + 1;
        const lineEndIndex = input.value.indexOf('\n', end);
        const lineEnd = lineEndIndex === -1 ? input.value.length : lineEndIndex;
        const selected = input.value.slice(lineStart, lineEnd);
        const updated = selected
          .split('\n')
          .map((line) => {
            if (line.startsWith(prefix)) {
              return line;
            }
            return `${prefix}${line}`;
          })
          .join('\n');
        input.setRangeText(updated, lineStart, lineEnd, 'end');
        input.focus();
      },

      insertCodeFence(input) {
        const start = input.selectionStart || 0;
        const end = input.selectionEnd || 0;
        const current = input.value.slice(start, end).trim();
        const block = current !== '' ? `\`\`\`\n${current}\n\`\`\`` : '```\ncode\n```';
        input.setRangeText(block, start, end, 'end');
        input.focus();
      },

      applyMarkdownAction(action, input, helper, preview) {
        if (action === 'write' || action === 'preview') {
          if (action === 'preview') {
            this.renderMarkdownPreview(input, preview);
          }
          this.setMarkdownMode(helper, action);
          return;
        }

        this.setMarkdownMode(helper, 'write');
        if (action === 'h2') {
          this.prefixSelectionLines(input, '## ');
        } else if (action === 'h3') {
          this.prefixSelectionLines(input, '### ');
        } else if (action === 'bold') {
          this.surroundSelection(input, '**', '**', 'bold text');
        } else if (action === 'italic') {
          this.surroundSelection(input, '*', '*', 'italic text');
        } else if (action === 'link') {
          this.surroundSelection(input, '[', '](https://example.com)', 'link text');
        } else if (action === 'quote') {
          this.prefixSelectionLines(input, '> ');
        } else if (action === 'ul') {
          this.prefixSelectionLines(input, '- ');
        } else if (action === 'code') {
          this.insertCodeFence(input);
        } else {
          return;
        }
        input.dispatchEvent(new Event('input', { bubbles: true }));
      },

      parseFlashPayload(html) {
        if (!html || typeof html !== 'string' || !html.includes('data-ui-flash')) {
          return null;
        }
        const host = document.createElement('div');
        host.innerHTML = html;
        const flash = host.querySelector('[data-ui-flash]');
        if (!flash) {
          return null;
        }
        const level = String(flash.dataset.level || 'info').toLowerCase();
        const message = String(flash.dataset.message || flash.textContent || '').trim();
        if (message === '') {
          return null;
        }
        return { level, message };
      },

      ensureToastStack() {
        const existing = document.getElementById('cms-toast-stack');
        if (existing) {
          return existing;
        }
        const created = document.createElement('div');
        created.id = 'cms-toast-stack';
        created.className = 'cms-toast-stack';
        created.setAttribute('aria-live', 'polite');
        created.setAttribute('aria-atomic', 'true');
        document.body.appendChild(created);
        return created;
      },

      showToast(level, message) {
        const stack = this.ensureToastStack();
        if (!stack || !message) {
          return;
        }
        const tone = level === 'error' ? 'danger' : level === 'success' ? 'success' : 'info';
        const toast = document.createElement('div');
        toast.className = `cms-toast cms-toast-${tone}`;
        toast.textContent = message;
        stack.appendChild(toast);
        window.setTimeout(() => {
          toast.remove();
        }, 3200);
      },

      refreshLibrary(scope) {
        const resolvedScope = toScope(scope || this.libraryScope);
        this.libraryScope = resolvedScope;
        this.loadPanel('#cms-library-panel', `/admin/partials/cms/library/${resolvedScope}`);
      },

      handleFeedbackResponse(event) {
        const detail = event && event.detail ? event.detail : null;
        const form = detail && detail.elt;
        if (!form || !detail || !detail.successful) {
          return;
        }

        const responseText =
          detail.xhr && typeof detail.xhr.responseText === 'string' ? detail.xhr.responseText : '';
        const flash = this.parseFlashPayload(responseText);
        if (!flash) {
          return;
        }

        this.showToast(flash.level, flash.message);
        if (flash.level === 'error') {
          return;
        }

        if (form.id === 'cms-editor-form') {
          const resourceField = form.querySelector('input[name="resource"]');
          const slugField = form.querySelector('input[name="slug"]');
          const originalField = form.querySelector('input[name="original_slug"]');
          if (!resourceField || !slugField || !originalField) {
            return;
          }

          const resource = toScope(resourceField.value || '');
          const nextSlug = String(slugField.value || '').trim();
          if (nextSlug === '') {
            return;
          }

          const previousSlug = String(originalField.value || '').trim();
          originalField.value = nextSlug;

          this.queueLibraryHighlight(nextSlug);
          this.refreshLibrary(resource);

          // On first create (or after slug rename), reopen editor in edit mode to prevent duplicate-create saves.
          if (previousSlug === '' || previousSlug !== nextSlug) {
            this.openEditor(resource, nextSlug);
          }
          return;
        }

        if (form.id === 'cms-transition-form' || form.classList.contains('cms-inline-form')) {
          const resourceField = form.querySelector('input[name="resource"]');
          const slugField = form.querySelector(
            'input[name="slug"], input[name="target_slug"], input[name="original_slug"]',
          );
          const slugValue = String((slugField && slugField.value) || '').trim();
          if (slugValue !== '') {
            this.queueLibraryHighlight(slugValue);
          }
          this.refreshLibrary(resourceField ? resourceField.value : this.libraryScope);
          return;
        }

        if (form.classList.contains('cms-delete-form')) {
          const action = String(form.getAttribute('hx-post') || form.getAttribute('action') || '');
          const resourceFromPath = action.match(/\/cms\/(pages|posts|categories)\//);
          if (resourceFromPath) {
            this.goLibrary(resourceFromPath[1]);
          }
        }
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
            target.innerHTML = '<div class="py-6 text-center text-sm text-base-content/60">Unable to load this panel.</div>';
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

      queueLibraryHighlight(slug) {
        this.pendingLibraryHighlightSlug = String(slug || '').trim();
      },

      applyPendingLibraryHighlight() {
        const slug = String(this.pendingLibraryHighlightSlug || '').trim();
        this.pendingLibraryHighlightSlug = '';
        if (slug === '') {
          return;
        }
        const rows = this.collectLibraryRows();
        let target = null;
        rows.forEach((row) => {
          if (String(row.dataset.slug || '') === slug) {
            target = row;
          }
        });
        if (!target) {
          return;
        }
        target.classList.add('cms-row-flash');
        window.setTimeout(() => {
          target.classList.remove('cms-row-flash');
        }, 1100);
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
        const typingTarget = isTypingTarget(event.target);

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

        if (!typingTarget && this.handleGotoSequence(event)) {
          event.preventDefault();
          return;
        }

        if (typingTarget && key !== '/') {
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
