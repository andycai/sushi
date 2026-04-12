use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub email: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub role: UserRole,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserRole {
    Admin,
    Editor,
    Viewer,
    Custom(String),
}

impl Default for UserRole {
    fn default() -> Self {
        Self::Viewer
    }
}

impl UserRole {
    pub fn from_slug(input: &str) -> Self {
        let normalized = input.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "admin" => Self::Admin,
            "editor" => Self::Editor,
            "viewer" | "" => Self::Viewer,
            _ => Self::Custom(normalized),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            UserRole::Admin => "admin",
            UserRole::Editor => "editor",
            UserRole::Viewer => "viewer",
            UserRole::Custom(slug) => slug.as_str(),
        }
    }

    pub fn is_admin(&self) -> bool {
        matches!(self, UserRole::Admin)
    }
}

impl std::fmt::Display for UserRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl Serialize for UserRole {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for UserRole {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Ok(UserRole::from_slug(&raw))
    }
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
}

#[cfg(test)]
mod tests {
    use super::UserRole;

    #[test]
    fn custom_role_round_trip_uses_slug() {
        let role = UserRole::from_slug("auditor");
        assert_eq!(role.as_str(), "auditor");
        assert_eq!(role.to_string(), "auditor");
    }

    #[test]
    fn role_deserialize_accepts_custom_role_slug() {
        let role: UserRole = serde_json::from_str("\"release-manager\"").expect("valid role");
        assert_eq!(role, UserRole::Custom("release-manager".to_string()));
    }
}
