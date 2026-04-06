use axum::response::Html;

pub async fn users_page() -> Html<&'static str> {
    Html("<html><head><meta charset=\"UTF-8\"><title>Users — Sushi Admin</title></head><body><h1>Users Management</h1><p>Coming soon...</p></body></html>")
}
