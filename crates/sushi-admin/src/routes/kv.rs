use axum::response::Html;

pub async fn kv_page() -> Html<&'static str> {
    Html(include_str!("../../templates/admin/kv.html"))
}
