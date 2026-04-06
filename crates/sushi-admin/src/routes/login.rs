use axum::response::Html;

pub async fn login_page() -> Html<&'static str> {
    Html(include_str!("../../templates/admin/login.html"))
}
