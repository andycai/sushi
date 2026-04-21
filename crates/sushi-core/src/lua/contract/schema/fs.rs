#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, Default)]
pub struct FsContract {
    pub operation: String,
    pub path: String,
}
