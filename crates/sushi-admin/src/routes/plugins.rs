use axum::response::Html;

pub async fn plugins_page() -> Html<&'static str> {
    Html(include_str!("../../templates/admin/plugins.html"))
}
