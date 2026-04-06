use axum::response::Html;

pub async fn logs_page() -> Html<&'static str> {
    Html("<html><head><meta charset=\"UTF-8\"><title>Logs — Sushi Admin</title></head><body><h1>Logs</h1><p>Coming soon...</p></body></html>")
}
