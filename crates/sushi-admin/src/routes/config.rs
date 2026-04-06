use axum::response::Html;

pub async fn config_page() -> Html<&'static str> {
    Html(
        r#"<!DOCTYPE html>
<html lang="en">
<head><meta charset="UTF-8"><meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Config — Sushi Admin</title>
<script defer src="https://unpkg.com/alpinejs@3.14.1/dist/cdn.min.js"></script>
<script src="https://cdn.jsdelivr.net/npm/@tailwindcss/browser@4"></script>
<body class="bg-gray-100 min-h-screen">
<div class="flex h-screen">
  <nav class="w-60 bg-gray-900 text-white flex-shrink-0">
    <div class="p-4 text-xl font-bold border-b border-gray-700">Sushi Admin</div>
    <div class="mt-4">
      <a href="/admin/" class="block px-4 py-2 hover:bg-gray-700">Dashboard</a>
      <a href="/admin/plugins" class="block px-4 py-2 hover:bg-gray-700">Plugins</a>
      <a href="/admin/users" class="block px-4 py-2 hover:bg-gray-700">Users</a>
      <a href="/admin/config" class="block px-4 py-2 bg-gray-700">Config</a>
      <a href="/admin/logs" class="block px-4 py-2 hover:bg-gray-700">Logs</a>
    </div>
  </nav>
  <main class="flex-1 p-6 overflow-auto" x-data="configPage()">
    <h1 class="text-2xl font-bold mb-6">Configuration</h1>

    <div x-show="!loaded" class="text-gray-500">Loading configuration...</div>
    <div x-show="loaded">
      <!-- Server Section -->
      <div class="mb-6">
        <h2 class="text-lg font-semibold mb-3 text-gray-800 border-b pb-1">Server</h2>
        <div class="bg-white rounded shadow overflow-hidden">
          <table class="w-full">
            <tbody>
              <tr class="border-b"><td class="px-4 py-2 text-sm font-medium text-gray-600 w-48">Host</td><td class="px-4 py-2 text-sm font-mono" x-text="config.server?.host"></td></tr>
              <tr class="border-b"><td class="px-4 py-2 text-sm font-medium text-gray-600">Port</td><td class="px-4 py-2 text-sm font-mono" x-text="config.server?.port"></td></tr>
            </tbody>
          </table>
        </div>
      </div>

      <!-- Database Section -->
      <div class="mb-6">
        <h2 class="text-lg font-semibold mb-3 text-gray-800 border-b pb-1">Database</h2>
        <div class="bg-white rounded shadow overflow-hidden">
          <table class="w-full">
            <tbody>
              <tr class="border-b"><td class="px-4 py-2 text-sm font-medium text-gray-600 w-48">Path</td><td class="px-4 py-2 text-sm font-mono" x-text="config.database?.path"></td></tr>
            </tbody>
          </table>
        </div>
      </div>

      <!-- JWT Section -->
      <div class="mb-6">
        <h2 class="text-lg font-semibold mb-3 text-gray-800 border-b pb-1">JWT</h2>
        <div class="bg-white rounded shadow overflow-hidden">
          <table class="w-full">
            <tbody>
              <tr class="border-b"><td class="px-4 py-2 text-sm font-medium text-gray-600 w-48">Secret</td><td class="px-4 py-2 text-sm font-mono text-red-500">••••••••</td></tr>
              <tr class="border-b"><td class="px-4 py-2 text-sm font-medium text-gray-600">Access TTL</td><td class="px-4 py-2 text-sm font-mono" x-text="config.jwt?.access_ttl + 's'"></td></tr>
              <tr class="border-b"><td class="px-4 py-2 text-sm font-medium text-gray-600">Refresh TTL</td><td class="px-4 py-2 text-sm font-mono" x-text="config.jwt?.refresh_ttl + 's'"></td></tr>
            </tbody>
          </table>
        </div>
      </div>

      <!-- Plugins Section -->
      <div class="mb-6">
        <h2 class="text-lg font-semibold mb-3 text-gray-800 border-b pb-1">Plugins</h2>
        <div class="bg-white rounded shadow overflow-hidden">
          <table class="w-full">
            <tbody>
              <tr class="border-b"><td class="px-4 py-2 text-sm font-medium text-gray-600 w-48">Directory</td><td class="px-4 py-2 text-sm font-mono" x-text="config.plugins?.directory"></td></tr>
            </tbody>
          </table>
        </div>
      </div>
    </div>
  </main>
</div>
<script>
function configPage() {
  return {
    config: {},
    loaded: false,
    async init() {
      try {
        const resp = await fetch('/api/config');
        if (resp.ok) this.config = await resp.json();
        else throw new Error();
      } catch { /* ignore */ }
      this.loaded = true;
    }
  };
}
</script>
</body></html>"#,
    )
}
