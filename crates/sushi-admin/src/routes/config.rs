use axum::response::Html;

pub async fn config_page() -> Html<&'static str> {
    Html("<html><head><meta charset=\"UTF-8\"><title>Config — Sushi Admin</title></head><body><h1>Config</h1><p>Coming soon...</p></body></html>")
}
