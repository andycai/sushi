(() => {
  window.usersPage = function usersPage() {
    return {
      users: [],
      showModal: false,
      error: null,
      newUser: { username: '', email: '', password: '', role: 'viewer' },
      async init() {
        await this.loadUsers();
      },
      async loadUsers() {
        try {
          const resp = await fetch('/api/users');
          if (!resp.ok) throw new Error('Failed to load users');
          const payload = await resp.json();
          this.users = Array.isArray(payload) ? payload : payload.users || [];
          this.error = null;
        } catch (e) {
          this.error = 'Could not load users. Is the server running?';
          this.users = [];
        }
      },
      async createUser() {
        try {
          const resp = await fetch('/api/users', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(this.newUser),
          });
          if (!resp.ok) throw new Error('Failed to create user');
          this.showModal = false;
          await this.loadUsers();
        } catch (e) {
          this.error = 'Failed to create user: ' + (e instanceof Error ? e.message : 'unknown error');
        }
      },
      async deleteUser(id) {
        if (!confirm('Delete this user?')) return;
        try {
          const resp = await fetch('/api/users/' + encodeURIComponent(id), { method: 'DELETE' });
          if (!resp.ok) throw new Error('Failed to delete user');
          await this.loadUsers();
        } catch (e) {
          this.error = 'Failed to delete user';
        }
      },
    };
  };
})();
