use axum::response::Html;

pub async fn dashboard_page() -> Html<&'static str> {
    Html(include_str!("../../templates/admin/dashboard.html"))
}
