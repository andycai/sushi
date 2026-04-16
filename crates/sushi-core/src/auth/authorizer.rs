use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct HttpBinding {
    pub surface: String,
    pub method: String,
    pub path_pattern: String,
    pub policy_key: String,
}

impl HttpBinding {
    pub fn matches(&self, surface: &str, method: &str, path: &str) -> bool {
        self.surface == surface
            && self.method.eq_ignore_ascii_case(method)
            && path_pattern_matches(&self.path_pattern, path)
    }
}

#[derive(Debug, Clone, Default)]
pub struct CompiledPolicySnapshot {
    pub http_bindings: Vec<HttpBinding>,
    pub command_bindings: HashMap<(String, String), String>,
    pub role_policy_keys: HashMap<String, HashSet<String>>,
}

impl CompiledPolicySnapshot {
    pub fn new(
        http_bindings: Vec<HttpBinding>,
        command_bindings: Vec<(String, String, String)>,
        role_grants: Vec<(String, String)>,
    ) -> Self {
        let mut snapshot = Self {
            http_bindings,
            command_bindings: HashMap::new(),
            role_policy_keys: HashMap::new(),
        };
        for (surface, command_name, policy_key) in command_bindings {
            snapshot
                .command_bindings
                .insert((surface, command_name), policy_key);
        }
        for (role, policy_key) in role_grants {
            snapshot
                .role_policy_keys
                .entry(role)
                .or_default()
                .insert(policy_key);
        }
        snapshot
    }

    pub fn from_raw(
        command_bindings: Vec<(&str, &str, &str)>,
        role_grants: Vec<(&str, &str)>,
    ) -> Self {
        Self::new(
            vec![],
            command_bindings
                .into_iter()
                .map(|(surface, command_name, policy_key)| {
                    (
                        surface.to_string(),
                        command_name.to_string(),
                        policy_key.to_string(),
                    )
                })
                .collect(),
            role_grants
                .into_iter()
                .map(|(role, policy_key)| (role.to_string(), policy_key.to_string()))
                .collect(),
        )
    }

    pub fn command_allowed(&self, role: &str, surface: &str, command_name: &str) -> bool {
        let Some(policy_key) = self
            .command_bindings
            .get(&(surface.to_string(), command_name.to_string()))
        else {
            return false;
        };

        self.role_policy_keys
            .get(role)
            .map(|grants| grants.contains(policy_key))
            .unwrap_or(false)
    }

    pub fn has_command_binding(&self, surface: &str, command_name: &str) -> bool {
        self.command_bindings
            .contains_key(&(surface.to_string(), command_name.to_string()))
    }

    pub fn http_allowed(&self, role: &str, surface: &str, method: &str, path: &str) -> bool {
        let Some(grants) = self.role_policy_keys.get(role) else {
            return false;
        };

        self.http_bindings
            .iter()
            .filter(|binding| binding.matches(surface, method, path))
            .any(|binding| grants.contains(&binding.policy_key))
    }
}

#[derive(Debug, Clone)]
pub struct Authorizer {
    snapshot: Arc<RwLock<CompiledPolicySnapshot>>,
}

impl Authorizer {
    pub fn new(snapshot: CompiledPolicySnapshot) -> Self {
        Self {
            snapshot: Arc::new(RwLock::new(snapshot)),
        }
    }

    pub async fn replace_snapshot(&self, snapshot: CompiledPolicySnapshot) {
        let mut writer = self.snapshot.write().await;
        *writer = snapshot;
    }

    pub async fn check_http(
        &self,
        role: &str,
        surface: &str,
        method: &str,
        path: &str,
    ) -> Result<(), String> {
        let snapshot = self.snapshot.read().await;
        if snapshot.http_allowed(role, surface, method, path) {
            Ok(())
        } else {
            Err(format!(
                "policy denied for role={role} target={surface}:{method} {path}"
            ))
        }
    }

    pub async fn check_command(
        &self,
        role: &str,
        surface: &str,
        command_name: &str,
    ) -> Result<(), String> {
        let snapshot = self.snapshot.read().await;
        if snapshot.command_allowed(role, surface, command_name) {
            Ok(())
        } else {
            Err(format!(
                "policy denied for role={role} command={surface}:{command_name}"
            ))
        }
    }

    pub async fn has_command_binding(&self, surface: &str, command_name: &str) -> bool {
        let snapshot = self.snapshot.read().await;
        snapshot.has_command_binding(surface, command_name)
    }
}

fn path_pattern_matches(pattern: &str, path: &str) -> bool {
    if let Some(prefix) = pattern.strip_suffix('*') {
        return path.starts_with(prefix);
    }

    let pattern_segments = split_path_segments(pattern);
    let path_segments = split_path_segments(path);

    if pattern_segments.len() != path_segments.len() {
        return false;
    }

    pattern_segments
        .iter()
        .zip(path_segments.iter())
        .all(|(pattern_segment, actual_segment)| {
            pattern_segment == actual_segment || is_path_param(pattern_segment)
        })
}

fn split_path_segments(path: &str) -> Vec<&str> {
    let trimmed = path.trim_matches('/');
    if trimmed.is_empty() {
        return Vec::new();
    }
    trimmed.split('/').collect()
}

fn is_path_param(segment: &str) -> bool {
    segment.starts_with('{') && segment.ends_with('}') && segment.len() > 2
}

#[cfg(test)]
mod tests {
    use super::{CompiledPolicySnapshot, HttpBinding};

    #[test]
    fn http_binding_matches_path_params() {
        let binding = HttpBinding {
            surface: "admin".to_string(),
            method: "GET".to_string(),
            path_pattern: "/admin/partials/users/{id}".to_string(),
            policy_key: "admin.users.read".to_string(),
        };

        assert!(binding.matches("admin", "GET", "/admin/partials/users/42"));
    }

    #[test]
    fn http_binding_matches_slash_star_wildcard_prefix() {
        let binding = HttpBinding {
            surface: "api".to_string(),
            method: "GET".to_string(),
            path_pattern: "/api/kv/*".to_string(),
            policy_key: "api.kv.read".to_string(),
        };

        assert!(binding.matches("api", "GET", "/api/kv/key"));
        assert!(binding.matches("api", "GET", "/api/kv/key/child"));
        assert!(!binding.matches("api", "GET", "/api/kv"));
    }

    #[test]
    fn http_binding_matches_trailing_star_wildcard_prefix() {
        let binding = HttpBinding {
            surface: "api".to_string(),
            method: "GET".to_string(),
            path_pattern: "/api/kv*".to_string(),
            policy_key: "api.kv.read".to_string(),
        };

        assert!(binding.matches("api", "GET", "/api/kv"));
        assert!(binding.matches("api", "GET", "/api/kv/nested"));
    }

    #[test]
    fn command_binding_requires_exact_name() {
        let snapshot = CompiledPolicySnapshot::from_raw(
            vec![("cli", "plugin:list", "cli.plugin.list.read")],
            vec![("editor", "cli.plugin.list.read")],
        );

        assert!(snapshot.command_allowed("editor", "cli", "plugin:list"));
        assert!(!snapshot.command_allowed("editor", "cli", "plugin:delete"));
    }

    #[test]
    fn command_binding_presence_lookup_is_exact() {
        let snapshot = CompiledPolicySnapshot::from_raw(
            vec![("cli", "plugin:list", "cli.plugin.list.read")],
            vec![],
        );

        assert!(snapshot.has_command_binding("cli", "plugin:list"));
        assert!(!snapshot.has_command_binding("cli", "plugin:delete"));
    }
}
