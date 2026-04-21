#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, Default)]
pub struct WebContract {
    pub kind: String,
    pub identifier: String,
}
