(() => {
  function defaultPageForm() {
    return {
      original_slug: '',
      title: '',
      slug: '',
      status: 'draft',
      markdown_body: '',
    };
  }

  function defaultPostForm() {
    return {
      original_slug: '',
      title: '',
      slug: '',
      excerpt: '',
      markdown_body: '',
      status: 'draft',
      category_slug: '',
    };
  }

  function defaultCategoryForm() {
    return {
      original_slug: '',
      name: '',
      slug: '',
      description: '',
    };
  }

  function readTextareaValue(row, selector) {
    if (!row) {
      return '';
    }
    const node = row.querySelector(selector);
    if (!node) {
      return '';
    }
    return typeof node.value === 'string' ? node.value : '';
  }

  function normalizeText(value) {
    return typeof value === 'string' ? value : '';
  }

  window.cmsPage = function cmsPage() {
    return {
      pageForm: defaultPageForm(),
      postForm: defaultPostForm(),
      categoryForm: defaultCategoryForm(),

      resetPageForm() {
        this.pageForm = defaultPageForm();
      },

      resetPostForm() {
        this.postForm = defaultPostForm();
      },

      resetCategoryForm() {
        this.categoryForm = defaultCategoryForm();
      },

      editPageFromRow(row) {
        if (!row) {
          return;
        }
        this.pageForm = {
          original_slug: normalizeText(row.dataset.pageSlug),
          title: normalizeText(row.dataset.pageTitle),
          slug: normalizeText(row.dataset.pageSlug),
          status: normalizeText(row.dataset.pageStatus) || 'draft',
          markdown_body: readTextareaValue(row, '[data-page-markdown]'),
        };
      },

      editPostFromRow(row) {
        if (!row) {
          return;
        }
        this.postForm = {
          original_slug: normalizeText(row.dataset.postSlug),
          title: normalizeText(row.dataset.postTitle),
          slug: normalizeText(row.dataset.postSlug),
          excerpt: readTextareaValue(row, '[data-post-excerpt]'),
          markdown_body: readTextareaValue(row, '[data-post-markdown]'),
          status: normalizeText(row.dataset.postStatus) || 'draft',
          category_slug: normalizeText(row.dataset.postCategorySlug),
        };
      },

      editCategoryFromRow(row) {
        if (!row) {
          return;
        }
        this.categoryForm = {
          original_slug: normalizeText(row.dataset.categorySlug),
          name: normalizeText(row.dataset.categoryName),
          slug: normalizeText(row.dataset.categorySlug),
          description: readTextareaValue(row, '[data-category-description]'),
        };
      },

      confirmDelete(kind, slug) {
        const entity = normalizeText(kind) || 'item';
        const label = normalizeText(slug) || 'this item';
        return window.confirm(`Delete ${entity} "${label}"? This action cannot be undone.`);
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

      notifyFeedback(selector, fallbackLevel) {
        if (window.AdminUI && typeof window.AdminUI.consumeFeedback === 'function') {
          window.AdminUI.consumeFeedback(selector, fallbackLevel);
        }
      },

      isSuccessfulRequest(event, feedbackSelector) {
        if (!event?.detail?.successful) {
          return false;
        }
        return !this.isErrorFeedback(feedbackSelector);
      },

      refreshPartial(url, target, errorMessage) {
        if (window.AdminUI && typeof window.AdminUI.refreshPartial === 'function') {
          window.AdminUI.refreshPartial({
            url,
            target,
            errorMessage,
          });
          return;
        }

        fetch(url)
          .then((response) => {
            if (!response.ok) {
              throw new Error(`refresh failed (${response.status})`);
            }
            return response.text();
          })
          .then((html) => {
            const node = document.querySelector(target);
            if (node) {
              node.innerHTML = html;
            }
          })
          .catch(() => {
            if (window.AdminUI && typeof window.AdminUI.notify === 'function') {
              window.AdminUI.notify({
                tone: 'danger',
                title: 'Refresh failed',
                message: errorMessage,
              });
            }
          });
      },

      refreshPages() {
        this.refreshPartial(
          '/admin/partials/cms/pages/table',
          '#cms-page-table',
          'Unable to refresh pages table.',
        );
      },

      refreshPosts() {
        this.refreshPartial(
          '/admin/partials/cms/posts/table',
          '#cms-post-table',
          'Unable to refresh posts table.',
        );
      },

      refreshCategories() {
        this.refreshPartial(
          '/admin/partials/cms/categories/table',
          '#cms-category-table',
          'Unable to refresh categories table.',
        );
      },

      onPagesUpsertAfterRequest(event) {
        const ok = this.isSuccessfulRequest(event, '#cms-feedback');
        this.notifyFeedback('#cms-feedback', ok ? 'success' : 'error');
        if (ok) {
          this.refreshPages();
          this.resetPageForm();
        }
      },

      onPagesDeleteAfterRequest(event) {
        const ok = this.isSuccessfulRequest(event, '#cms-feedback');
        this.notifyFeedback('#cms-feedback', ok ? 'success' : 'error');
        if (ok) {
          this.refreshPages();
          this.resetPageForm();
        }
      },

      onPostsUpsertAfterRequest(event) {
        const ok = this.isSuccessfulRequest(event, '#cms-feedback');
        this.notifyFeedback('#cms-feedback', ok ? 'success' : 'error');
        if (ok) {
          this.refreshPosts();
          this.resetPostForm();
        }
      },

      onPostsDeleteAfterRequest(event) {
        const ok = this.isSuccessfulRequest(event, '#cms-feedback');
        this.notifyFeedback('#cms-feedback', ok ? 'success' : 'error');
        if (ok) {
          this.refreshPosts();
          this.resetPostForm();
        }
      },

      onCategoriesUpsertAfterRequest(event) {
        const ok = this.isSuccessfulRequest(event, '#cms-feedback');
        this.notifyFeedback('#cms-feedback', ok ? 'success' : 'error');
        if (ok) {
          this.refreshCategories();
          this.resetCategoryForm();
        }
      },

      onCategoriesDeleteAfterRequest(event) {
        const ok = this.isSuccessfulRequest(event, '#cms-feedback');
        this.notifyFeedback('#cms-feedback', ok ? 'success' : 'error');
        if (ok) {
          this.refreshCategories();
          this.resetCategoryForm();
        }
      },
    };
  };
})();
