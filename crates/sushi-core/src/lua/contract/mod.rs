pub mod schema {
    pub mod admin;
    pub mod api;
    pub mod cli;
    pub mod db;
    pub mod event;
    pub mod fs;
    pub mod menu;
    pub mod web;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractSchemaVersion {
    V2,
}

impl ContractSchemaVersion {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::V2 => "v2",
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default, PartialEq)]
pub struct LuaCapabilityContract {
    #[serde(default)]
    pub entries: Vec<LuaCapabilityEntry>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(tag = "surface", rename_all = "snake_case")]
pub enum LuaCapabilityEntry {
    Api(schema::api::ApiRouteContract),
    Admin(schema::admin::AdminPageContract),
    Cli(schema::cli::CliCommandContract),
    Menu(schema::menu::MenuContributionContract),
    Web(schema::web::WebContract),
    Db(schema::db::DbContract),
    Event(schema::event::EventContract),
    Fs(schema::fs::FsContract),
}
