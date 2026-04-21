#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, Default)]
pub struct ApiRouteContract {
    pub method: String,
    pub path: String,
}
