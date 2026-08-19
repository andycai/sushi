#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, Default)]
pub struct AdminPageContract {
    pub path: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub handler_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bundles: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub js: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub css: Vec<String>,
}
