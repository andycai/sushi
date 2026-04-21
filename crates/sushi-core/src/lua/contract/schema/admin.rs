#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, Default)]
pub struct AdminPageContract {
    pub path: String,
    pub title: String,
}
