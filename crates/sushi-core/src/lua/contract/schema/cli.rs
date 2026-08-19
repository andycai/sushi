#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, Default)]
pub struct CliCommandContract {
    pub name: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub handler_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy: Option<String>,
}
