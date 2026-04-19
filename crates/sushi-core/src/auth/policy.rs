#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyKey {
    pub key: String,
    pub surface: String,
    pub resource: String,
    pub action: String,
}

impl PolicyKey {
    pub fn parse(key: &str) -> Result<Self, String> {
        let normalized = key.trim().to_ascii_lowercase();
        let mut segments = normalized.split('.');
        let surface = segments
            .next()
            .ok_or_else(|| "policy key must be in surface.resource.action format".to_string())?;
        let resource = segments
            .next()
            .ok_or_else(|| "policy key must be in surface.resource.action format".to_string())?;
        let action = segments
            .next()
            .ok_or_else(|| "policy key must be in surface.resource.action format".to_string())?;

        if segments.next().is_some() {
            return Err("policy key must be in surface.resource.action format".to_string());
        }

        if !is_valid_segment(surface) || !is_valid_segment(resource) || !is_valid_segment(action) {
            return Err(
                "policy key segments must contain only ascii letters, digits, '_' or '-'"
                    .to_string(),
            );
        }

        Ok(Self {
            key: format!("{surface}.{resource}.{action}"),
            surface: surface.to_string(),
            resource: resource.to_string(),
            action: action.to_string(),
        })
    }
}

fn is_valid_segment(segment: &str) -> bool {
    !segment.is_empty()
        && segment
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '-')
}

#[cfg(test)]
mod tests {
    use super::PolicyKey;

    #[test]
    fn parses_policy_key_in_expected_format() {
        let parsed = PolicyKey::parse("admin.users.read").expect("policy key should parse");
        assert_eq!(parsed.key, "admin.users.read");
        assert_eq!(parsed.surface, "admin");
        assert_eq!(parsed.resource, "users");
        assert_eq!(parsed.action, "read");
    }

    #[test]
    fn parser_normalizes_case_and_whitespace() {
        let parsed = PolicyKey::parse("  Admin.Users.Read  ").expect("policy key should parse");
        assert_eq!(parsed.key, "admin.users.read");
    }

    #[test]
    fn rejects_policy_key_with_invalid_shape() {
        let err = PolicyKey::parse("users.read").expect_err("key with 2 segments should fail");
        assert!(err.contains("surface.resource.action"));
    }

    #[test]
    fn rejects_policy_key_with_invalid_characters() {
        let err =
            PolicyKey::parse("admin.users.re/ad").expect_err("invalid characters should fail");
        assert!(err.contains("segments"));
    }
}
