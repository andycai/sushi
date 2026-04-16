use anyhow::{anyhow, Result};
use sushi_core::auth::authorizer::Authorizer;
use sushi_core::context::SushiContext;

const DEFAULT_CLI_ROLE: &str = "admin";

pub fn resolve_cli_role(flag_role: Option<&str>, env_role: Option<&str>) -> String {
    normalize_role(flag_role)
        .or_else(|| normalize_role(env_role))
        .unwrap_or_else(|| DEFAULT_CLI_ROLE.to_string())
}

pub async fn ensure_command_authorized(
    ctx: &SushiContext,
    role: &str,
    command_target: &str,
) -> Result<()> {
    ensure_command_authorized_with_authorizer(&ctx.authorizer, role, command_target).await
}

async fn ensure_command_authorized_with_authorizer(
    authorizer: &Authorizer,
    role: &str,
    command_target: &str,
) -> Result<()> {
    let resolved_role = resolve_cli_role(Some(role), None);
    match authorizer
        .check_command(&resolved_role, "cli", command_target)
        .await
    {
        Ok(()) => Ok(()),
        Err(_)
            if resolved_role == DEFAULT_CLI_ROLE
                && !authorizer.has_command_binding("cli", command_target).await =>
        {
            Ok(())
        }
        Err(err) => Err(anyhow!(
            "authorization denied for command '{command_target}': {err}"
        )),
    }
}

fn normalize_role(input: Option<&str>) -> Option<String> {
    let value = input?.trim();
    if value.is_empty() {
        return None;
    }
    Some(value.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::{ensure_command_authorized_with_authorizer, resolve_cli_role};
    use sushi_core::auth::authorizer::{Authorizer, CompiledPolicySnapshot};

    #[test]
    fn resolves_role_from_flag_then_env_then_default() {
        assert_eq!(resolve_cli_role(Some("editor"), None), "editor");
        assert_eq!(resolve_cli_role(None, Some("viewer")), "viewer");
        assert_eq!(resolve_cli_role(None, None), "admin");
    }

    #[test]
    fn ignores_blank_role_inputs() {
        assert_eq!(resolve_cli_role(Some("   "), Some("viewer")), "viewer");
        assert_eq!(resolve_cli_role(Some("   "), Some("   ")), "admin");
    }

    #[test]
    fn normalizes_role_to_lowercase() {
        assert_eq!(resolve_cli_role(Some("  Admin "), None), "admin");
    }

    #[tokio::test]
    async fn allows_command_when_role_has_grant() {
        let authorizer = Authorizer::new(CompiledPolicySnapshot::from_raw(
            vec![("cli", "plugin:list", "cli.plugin.list.read")],
            vec![("editor", "cli.plugin.list.read")],
        ));
        let result =
            ensure_command_authorized_with_authorizer(&authorizer, "editor", "plugin:list").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn denies_command_when_binding_exists_without_grant() {
        let authorizer = Authorizer::new(CompiledPolicySnapshot::from_raw(
            vec![("cli", "plugin:list", "cli.plugin.list.read")],
            vec![],
        ));
        let result =
            ensure_command_authorized_with_authorizer(&authorizer, "admin", "plugin:list").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn allows_admin_when_command_binding_is_missing() {
        let authorizer = Authorizer::new(CompiledPolicySnapshot::from_raw(vec![], vec![]));
        let result =
            ensure_command_authorized_with_authorizer(&authorizer, "admin", "plugin:list").await;
        assert!(result.is_ok());
    }
}
