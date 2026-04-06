use axum::response::Html;

pub async fn logs_page() -> Html<&'static str> {
    Html(include_str!("../../templates/admin/logs.html"))
}
