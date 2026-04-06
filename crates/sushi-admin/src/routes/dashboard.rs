use axum::response::Html;

pub async fn dashboard_page() -> Html<&'static str> {
    Html(
        r#"<!DOCTYPE html>
<html lang="en">
<head><meta charset="UTF-8"><meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Sushi Admin</title>
<script defer src="https://unpkg.com/alpinejs@3.14.1/dist/cdn.min.js"></script>
<script src="https://cdn.jsdelivr.net/npm/@tailwindcss/browser@4"></script>
<body class="bg-gray-100 min-h-screen" x-data="adminApp()">
<div class="flex h-screen">
  <nav class="w-60 bg-gray-900 text-white flex-shrink-0">
    <div class="p-4 text-xl font-bold border-b border-gray-700">Sushi Admin</div>
    <div class="mt-4">
      <a href="/admin/" class="block px-4 py-2 bg-gray-700">Dashboard</a>
      <a href="/admin/plugins" class="block px-4 py-2 hover:bg-gray-700">Plugins</a>
      <a href="/admin/users" class="block px-4 py-2 hover:bg-gray-700">Users</a>
      <a href="/admin/config" class="block px-4 py-2 hover:bg-gray-700">Config</a>
      <a href="/admin/logs" class="block px-4 py-2 hover:bg-gray-700">Logs</a>
    </div>
  </nav>
  <main class="flex-1 p-6 overflow-auto">
    <h1 class="text-2xl font-bold mb-6">Dashboard</h1>
    <div class="grid grid-cols-3 gap-4">
      <div class="bg-white p-4 rounded shadow">
        <h3 class="text-gray-500">Plugins Loaded</h3>
        <p class="text-3xl font-bold" x-text="$store.stats.plugins">0</p>
      </div>
      <div class="bg-white p-4 rounded shadow">
        <h3 class="text-gray-500">Total Users</h3>
        <p class="text-3xl font-bold" x-text="$store.stats.users">0</p>
      </div>
      <div class="bg-white p-4 rounded shadow">
        <h3 class="text-gray-500">Uptime</h3>
        <p class="text-3xl font-bold" x-text="$store.stats.uptime">-</p>
      </div>
    </div>
  </main>
</div>
<script>
function adminApp() {
  Alpine.store('stats', { plugins: 0, users: 0, uptime: '-' });
  return {};
}
</script>
</body></html>"#,
    )
}
