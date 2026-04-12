(() => {
  async function handleLogin(event) {
    event.preventDefault();
    const form = event.currentTarget;
    const btn = document.getElementById('submitBtn');
    const err = document.getElementById('error');
    if (!btn || !err) return;
    btn.disabled = true;
    btn.textContent = 'Signing in...';
    err.classList.add('hidden');
    try {
      const resp = await fetch('/api/auth/login', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          username: document.getElementById('username')?.value,
          password: document.getElementById('password')?.value,
        }),
      });
      const data = await resp.json();
      if (!resp.ok) {
        throw new Error(data.error || 'Login failed');
      }
      document.cookie =
        'sushi_token=' + data.access_token + '; path=/; SameSite=Lax; max-age=86400';
      window.location.href = '/admin';
    } catch (error) {
      err.textContent = error instanceof Error ? error.message : 'Login failed';
      err.classList.remove('hidden');
      btn.disabled = false;
      btn.textContent = 'Sign In';
    }
  }

  document.addEventListener('DOMContentLoaded', () => {
    const form = document.getElementById('loginForm');
    if (form) {
      form.addEventListener('submit', handleLogin);
    }
  });
})();
