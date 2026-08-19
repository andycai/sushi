use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_REGISTRATION_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PluginId(String);

impl PluginId {
    pub fn new(value: impl Into<String>) -> Result<Self, String> {
        validate_identity(value.into()).map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PluginId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PluginInstanceId(String);

impl PluginInstanceId {
    pub fn new(value: impl Into<String>) -> Result<Self, String> {
        validate_identity(value.into()).map(Self)
    }

    pub fn legacy(plugin_name: &str) -> Self {
        Self(format!("legacy:{plugin_name}"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PluginInstanceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RegistrationId(u64);

impl RegistrationId {
    pub(crate) fn next() -> Self {
        Self(NEXT_REGISTRATION_ID.fetch_add(1, Ordering::Relaxed))
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

fn validate_identity(value: String) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("plugin identity must not be empty".to_string());
    }
    if trimmed != value {
        return Err("plugin identity must not contain surrounding whitespace".to_string());
    }
    if value.chars().any(char::is_control) {
        return Err("plugin identity must not contain control characters".to_string());
    }
    Ok(value)
}
