use axum::response::Html;

pub async fn login_page() -> Html<&'static str> {
    Html(r#"<!DOCTYPE html>
<html lang="en">
<head><meta charset="UTF-8"><meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Login — Sushi Admin</title>
<style>
*{box-sizing:border-box;margin:0;padding:0}
body{min-height:100vh;display:flex;align-items:center;justify-content:center;background:#f3f4f6;font-family:system-ui,sans-serif}
.card{background:#fff;border-radius:8px;box-shadow:0 4px 6px -1px rgba(0,0,0,0.1);padding:2rem;width:100%;max-width:400px}
h1{font-size:1.5rem;font-weight:700;margin-bottom:1.5rem;text-align:center;color:#111}
.field{margin-bottom:1rem}
label{display:block;font-size:0.875rem;font-weight:500;color:#374151;margin-bottom:0.25rem}
input{width:100%;padding:0.5rem 0.75rem;border:1px solid #d1d5db;border-radius:4px;font-size:0.875rem}
input:focus{outline:none;border-color:#3b82f6;box-shadow:0 0 0 3px rgba(59,130,246,0.1)}
.btn{width:100%;padding:0.625rem;background:#2563eb;color:#fff;border:none;border-radius:4px;font-weight:500;font-size:0.875rem;cursor:pointer;margin-top:0.5rem}
.btn:hover{background:#1d4ed8}
.error{background:#fef2f2;color:#dc2626;padding:0.75rem;border-radius:4px;font-size:0.875rem;margin-bottom:1rem;display:none}
</style>
</head>
<body>
<div class="card">
  <h1>Sushi Admin</h1>
  <div class="error" id="error"></div>
  <form id="loginForm" onsubmit="handleLogin(event)">
    <div class="field">
      <label for="username">Username</label>
      <input type="text" id="username" name="username" required autocomplete="username">
    </div>
    <div class="field">
      <label for="password">Password</label>
      <input type="password" id="password" name="password" required autocomplete="current-password">
    </div>
    <button type="submit" class="btn" id="submitBtn">Sign In</button>
  </form>
</div>
<script>
async function handleLogin(e) {
  e.preventDefault();
  const btn = document.getElementById('submitBtn');
  const err = document.getElementById('error');
  btn.disabled = true;
  btn.textContent = 'Signing in...';
  err.style.display = 'none';
  try {
    const res = await fetch('/api/auth/login', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        username: document.getElementById('username').value,
        password: document.getElementById('password').value
      })
    });
    const data = await res.json();
    if (!res.ok) throw new Error(data.error || 'Login failed');
    // Set auth cookie (sent automatically with subsequent requests)
    document.cookie = 'sushi_token=' + data.access_token + '; path=/; SameSite=Lax; max-age=86400';
    window.location.href = '/admin';
  } catch(e) {
    err.textContent = e.message;
    err.style.display = 'block';
    btn.disabled = false;
    btn.textContent = 'Sign In';
  }
}
</script>
</body></html>"#)
}
