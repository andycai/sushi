use axum::response::Html;

pub async fn config_page() -> Html<&'static str> {
    Html(include_str!("../../templates/admin/config.html"))
}
