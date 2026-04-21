#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, Default)]
pub struct ApiRouteContract {
    pub method: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub handler_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub public: bool,
}

const fn is_false(value: &bool) -> bool {
    !*value
}
