use axum::response::Html;

pub async fn users_page() -> Html<&'static str> {
    Html(
        r#"<!DOCTYPE html>
<html lang="en">
<head><meta charset="UTF-8"><meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Users — Sushi Admin</title>
<script defer src="https://unpkg.com/alpinejs@3.14.1/dist/cdn.min.js"></script>
<script src="https://cdn.jsdelivr.net/npm/@tailwindcss/browser@4"></script>
<body class="bg-gray-100 min-h-screen">
<div class="flex h-screen">
  <nav class="w-60 bg-gray-900 text-white flex-shrink-0">
    <div class="p-4 text-xl font-bold border-b border-gray-700">Sushi Admin</div>
    <div class="mt-4">
      <a href="/admin/" class="block px-4 py-2 hover:bg-gray-700">Dashboard</a>
      <a href="/admin/plugins" class="block px-4 py-2 hover:bg-gray-700">Plugins</a>
      <a href="/admin/users" class="block px-4 py-2 bg-gray-700">Users</a>
      <a href="/admin/config" class="block px-4 py-2 hover:bg-gray-700">Config</a>
      <a href="/admin/logs" class="block px-4 py-2 hover:bg-gray-700">Logs</a>
    </div>
  </nav>
  <main class="flex-1 p-6 overflow-auto" x-data="usersPage()">
    <div class="flex justify-between items-center mb-6">
      <h1 class="text-2xl font-bold">Users</h1>
      <button @click="showModal = true; newUser = {username:'',email:'',password:'',role:'viewer'}"
        class="bg-blue-600 text-white px-4 py-2 rounded hover:bg-blue-700">Add User</button>
    </div>

    <div x-show="error" class="mb-4 p-3 bg-red-100 text-red-700 rounded" x-text="error"></div>

    <div class="bg-white rounded shadow overflow-hidden">
      <table class="w-full">
        <thead class="bg-gray-50 border-b">
          <tr>
            <th class="px-4 py-3 text-left text-sm font-medium text-gray-600">ID</th>
            <th class="px-4 py-3 text-left text-sm font-medium text-gray-600">Username</th>
            <th class="px-4 py-3 text-left text-sm font-medium text-gray-600">Email</th>
            <th class="px-4 py-3 text-left text-sm font-medium text-gray-600">Role</th>
            <th class="px-4 py-3 text-center text-sm font-medium text-gray-600">Actions</th>
          </tr>
        </thead>
        <tbody>
          <template x-for="u in users" :key="u.id">
            <tr class="border-b hover:bg-gray-50">
              <td class="px-4 py-3 text-sm" x-text="u.id"></td>
              <td class="px-4 py-3 text-sm font-medium" x-text="u.username"></td>
              <td class="px-4 py-3 text-sm text-gray-600" x-text="u.email"></td>
              <td class="px-4 py-3">
                <span class="px-2 py-1 rounded text-xs font-medium"
                  :class="u.role === 'admin' ? 'bg-purple-100 text-purple-700' : u.role === 'editor' ? 'bg-blue-100 text-blue-700' : 'bg-gray-100 text-gray-700'"
                  x-text="u.role"></span>
              </td>
              <td class="px-4 py-3 text-center">
                <button @click="deleteUser(u.id)" class="text-red-600 hover:text-red-800 text-sm font-medium">Delete</button>
              </td>
            </tr>
          </template>
          <tr x-show="users.length === 0">
            <td colspan="5" class="px-4 py-8 text-center text-gray-500">No users found. Add one to get started.</td>
          </tr>
        </tbody>
      </table>
    </div>

    <!-- Add User Modal -->
    <div x-show="showModal" class="fixed inset-0 bg-black/50 flex items-center justify-center z-50" style="display:none">
      <div class="bg-white rounded-lg shadow-xl w-full max-w-md mx-4">
        <div class="px-6 py-4 border-b">
          <h2 class="text-lg font-semibold">Add User</h2>
        </div>
        <form @submit.prevent="createUser" class="px-6 py-4 space-y-4">
          <div>
            <label class="block text-sm font-medium text-gray-700 mb-1">Username</label>
            <input x-model="newUser.username" type="text" required
              class="w-full border rounded px-3 py-2 focus:ring-2 focus:ring-blue-500 focus:outline-none">
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 mb-1">Email</label>
            <input x-model="newUser.email" type="email" required
              class="w-full border rounded px-3 py-2 focus:ring-2 focus:ring-blue-500 focus:outline-none">
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 mb-1">Password</label>
            <input x-model="newUser.password" type="password" required
              class="w-full border rounded px-3 py-2 focus:ring-2 focus:ring-blue-500 focus:outline-none">
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 mb-1">Role</label>
            <select x-model="newUser.role"
              class="w-full border rounded px-3 py-2 focus:ring-2 focus:ring-blue-500 focus:outline-none">
              <option value="viewer">Viewer</option>
              <option value="editor">Editor</option>
              <option value="admin">Admin</option>
            </select>
          </div>
          <div class="flex justify-end gap-3 pt-2">
            <button type="button" @click="showModal = false"
              class="px-4 py-2 border rounded hover:bg-gray-50">Cancel</button>
            <button type="submit"
              class="px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700">Create</button>
          </div>
        </form>
      </div>
    </div>
  </main>
</div>
<script>
function usersPage() {
  return {
    users: [],
    showModal: false,
    error: null,
    newUser: { username: '', email: '', password: '', role: 'viewer' },
    async init() { await this.loadUsers(); },
    async loadUsers() {
      try {
        const resp = await fetch('/api/users');
        if (!resp.ok) throw new Error('Failed to load users');
        this.users = await resp.json();
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
          body: JSON.stringify(this.newUser)
        });
        if (!resp.ok) throw new Error('Failed to create user');
        this.showModal = false;
        await this.loadUsers();
      } catch (e) {
        this.error = 'Failed to create user: ' + e.message;
      }
    },
    async deleteUser(id) {
      if (!confirm('Delete this user?')) return;
      try {
        await fetch('/api/users/' + id, { method: 'DELETE' });
        await this.loadUsers();
      } catch (e) {
        this.error = 'Failed to delete user';
      }
    }
  };
}
</script>
</body></html>"#,
    )
}
