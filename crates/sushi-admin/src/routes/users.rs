use axum::response::Html;

pub async fn users_page() -> Html<&'static str> {
    Html(include_str!("../../templates/admin/users.html"))
}
