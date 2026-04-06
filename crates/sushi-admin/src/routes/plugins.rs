use axum::response::Html;

pub async fn plugins_page() -> Html<&'static str> {
    Html(r#"<!DOCTYPE html>
<html lang="en"><head><meta charset="UTF-8"><meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Plugins — Sushi Admin</title>
<script defer src="https://cdn.jsdelivr.net/npm/alpinejs@3.x.x/dist/cdn.min.js"></script>
<link href="https://cdn.tailwindcss.com" rel="stylesheet"></head>
<body class="bg-gray-100 min-h-screen">
<div class="flex h-screen">
  <nav class="w-60 bg-gray-900 text-white flex-shrink-0">
    <div class="p-4 text-xl font-bold border-b border-gray-700">Sushi Admin</div>
    <div class="mt-4">
      <a href="/admin/" class="block px-4 py-2 hover:bg-gray-700">Dashboard</a>
      <a href="/admin/plugins" class="block px-4 py-2 bg-gray-700">Plugins</a>
      <a href="/admin/users" class="block px-4 py-2 hover:bg-gray-700">Users</a>
      <a href="/admin/config" class="block px-4 py-2 hover:bg-gray-700">Config</a>
      <a href="/admin/logs" class="block px-4 py-2 hover:bg-gray-700">Logs</a>
    </div>
  </nav>
  <main class="flex-1 p-6 overflow-auto" x-data="pluginsPage()">
    <h1 class="text-2xl font-bold mb-6">Plugins</h1>
    <div class="bg-white rounded shadow">
      <table class="w-full">
        <thead><tr class="bg-gray-50 border-b">
          <th class="px-4 py-2 text-left">Name</th>
          <th class="px-4 py-2 text-left">Version</th>
          <th class="px-4 py-2 text-left">Description</th>
          <th class="px-4 py-2">Status</th>
        </tr></thead>
        <tbody>
          <template x-for="p in plugins" :key="p.name">
            <tr class="border-b hover:bg-gray-50">
              <td class="px-4 py-2" x-text="p.name"></td>
              <td class="px-4 py-2" x-text="p.version"></td>
              <td class="px-4 py-2" x-text="p.description"></td>
              <td class="px-4 py-2 text-center">
                <span class="px-2 py-1 rounded text-sm" :class="p.loaded ? 'bg-green-100 text-green-700' : 'bg-red-100 text-red-700'" x-text="p.loaded ? 'Active' : 'Inactive'"></span>
              </td>
            </tr>
          </template>
        </tbody>
      </table>
    </div>
  </main>
</div>
<script>
function pluginsPage() {
  return {
    plugins: [],
    async init() {
      const resp = await fetch('/admin/api/plugins');
      this.plugins = await resp.json();
    }
  };
}
</script>
</body></html>"#)
}
