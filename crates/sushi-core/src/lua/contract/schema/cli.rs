#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, Default)]
pub struct CliCommandContract {
    pub name: String,
    pub description: String,
}
