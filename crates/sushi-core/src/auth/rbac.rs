use crate::storage::Storage;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Role {
    pub id: i64,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub is_system: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Permission {
    pub id: i64,
    pub slug: String,
    pub name: String,
    pub module: String,
    pub description: String,
    pub is_system: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RoleSummary {
    pub id: i64,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub is_system: bool,
    pub permission_count: i64,
    pub user_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PermissionSummary {
    pub id: i64,
    pub slug: String,
    pub name: String,
    pub module: String,
    pub description: String,
    pub is_system: bool,
    pub role_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RolePermissionAssignment {
    pub id: i64,
    pub slug: String,
    pub name: String,
    pub module: String,
    pub description: String,
    pub is_system: bool,
    pub assigned: bool,
}

pub struct RbacRepository {
    storage: Arc<dyn Storage>,
}

impl RbacRepository {
    pub fn new(storage: Arc<dyn Storage>) -> Self {
        Self { storage }
    }

    pub async fn list_roles(&self) -> Result<Vec<RoleSummary>, String> {
        let rows = self
            .storage
            .query(
                r#"
                SELECT
                    r.id,
                    r.slug,
                    r.name,
                    r.description,
                    r.is_system,
                    COUNT(DISTINCT rp.permission_id) AS permission_count,
                    COUNT(DISTINCT u.id) AS user_count
                FROM roles r
                LEFT JOIN role_permissions rp ON rp.role_id = r.id
                LEFT JOIN users u ON u.role = r.slug
                GROUP BY r.id
                ORDER BY r.is_system DESC, r.slug ASC
                "#,
                vec![],
            )
            .await
            .map_err(|err| err.to_string())?;

        rows.into_iter().map(row_to_role_summary).collect()
    }

    pub async fn list_permissions(&self) -> Result<Vec<PermissionSummary>, String> {
        let rows = self
            .storage
            .query(
                r#"
                SELECT
                    p.id,
                    p.slug,
                    p.name,
                    p.module,
                    p.description,
                    p.is_system,
                    COUNT(DISTINCT rp.role_id) AS role_count
                FROM permissions p
                LEFT JOIN role_permissions rp ON rp.permission_id = p.id
                GROUP BY p.id
                ORDER BY p.is_system DESC, p.module ASC, p.slug ASC
                "#,
                vec![],
            )
            .await
            .map_err(|err| err.to_string())?;

        rows.into_iter().map(row_to_permission_summary).collect()
    }

    pub async fn find_role(&self, id: i64) -> Result<Option<Role>, String> {
        let rows = self
            .storage
            .query(
                "SELECT * FROM roles WHERE id = ?1",
                vec![Value::Number(id.into())],
            )
            .await
            .map_err(|err| err.to_string())?;

        match rows.into_iter().next() {
            Some(row) => Ok(Some(row_to_role(row)?)),
            None => Ok(None),
        }
    }

    pub async fn find_role_by_slug(&self, slug: &str) -> Result<Option<Role>, String> {
        let rows = self
            .storage
            .query(
                "SELECT * FROM roles WHERE slug = ?1",
                vec![Value::String(slug.trim().to_ascii_lowercase())],
            )
            .await
            .map_err(|err| err.to_string())?;

        match rows.into_iter().next() {
            Some(row) => Ok(Some(row_to_role(row)?)),
            None => Ok(None),
        }
    }

    pub async fn find_permission(&self, id: i64) -> Result<Option<Permission>, String> {
        let rows = self
            .storage
            .query(
                "SELECT * FROM permissions WHERE id = ?1",
                vec![Value::Number(id.into())],
            )
            .await
            .map_err(|err| err.to_string())?;

        match rows.into_iter().next() {
            Some(row) => Ok(Some(row_to_permission(row)?)),
            None => Ok(None),
        }
    }

    pub async fn update_role(
        &self,
        id: i64,
        name: &str,
        description: &str,
    ) -> Result<Role, String> {
        self.storage
            .execute(
                r#"
                UPDATE roles
                SET name = ?1, description = ?2, updated_at = datetime('now')
                WHERE id = ?3
                "#,
                vec![
                    Value::String(name.to_string()),
                    Value::String(description.to_string()),
                    Value::Number(id.into()),
                ],
            )
            .await
            .map_err(|err| err.to_string())?;

        self.find_role(id)
            .await?
            .ok_or_else(|| "Role not found".to_string())
    }

    pub async fn create_role(
        &self,
        slug: &str,
        name: &str,
        description: &str,
    ) -> Result<Role, String> {
        let normalized_slug = slug.trim().to_ascii_lowercase();
        self.storage
            .execute(
                r#"
                INSERT INTO roles (slug, name, description)
                VALUES (?1, ?2, ?3)
                "#,
                vec![
                    Value::String(normalized_slug.clone()),
                    Value::String(name.to_string()),
                    Value::String(description.to_string()),
                ],
            )
            .await
            .map_err(|err| err.to_string())?;

        self.find_role_by_slug(&normalized_slug)
            .await?
            .ok_or_else(|| "Role not found after insert".to_string())
    }

    pub async fn delete_role(&self, id: i64) -> Result<(), String> {
        let role = self
            .find_role(id)
            .await?
            .ok_or_else(|| "Role not found".to_string())?;

        if role.is_system {
            return Err("System roles cannot be deleted".to_string());
        }

        let usage_rows = self
            .storage
            .query(
                "SELECT COUNT(*) AS count FROM users WHERE role = ?1",
                vec![Value::String(role.slug.clone())],
            )
            .await
            .map_err(|err| err.to_string())?;
        let user_count = usage_rows
            .first()
            .and_then(|row| row.get("count"))
            .and_then(Value::as_i64)
            .unwrap_or_default();
        if user_count > 0 {
            return Err("Role is assigned to existing users and cannot be deleted".to_string());
        }

        self.storage
            .execute(
                "DELETE FROM roles WHERE id = ?1",
                vec![Value::Number(id.into())],
            )
            .await
            .map_err(|err| err.to_string())?;
        Ok(())
    }

    pub async fn list_permissions_for_role(
        &self,
        role_id: i64,
    ) -> Result<Vec<RolePermissionAssignment>, String> {
        let rows = self
            .storage
            .query(
                r#"
                SELECT
                    p.id,
                    p.slug,
                    p.name,
                    p.module,
                    p.description,
                    p.is_system,
                    CASE WHEN rp.role_id IS NULL THEN 0 ELSE 1 END AS assigned
                FROM permissions p
                LEFT JOIN role_permissions rp
                    ON rp.permission_id = p.id
                   AND rp.role_id = ?1
                ORDER BY p.is_system DESC, p.module ASC, p.slug ASC
                "#,
                vec![Value::Number(role_id.into())],
            )
            .await
            .map_err(|err| err.to_string())?;

        rows.into_iter()
            .map(row_to_role_permission_assignment)
            .collect()
    }

    pub async fn replace_role_permissions(
        &self,
        role_id: i64,
        permission_ids: &[i64],
    ) -> Result<(), String> {
        if self.find_role(role_id).await?.is_none() {
            return Err("Role not found".to_string());
        }

        for permission_id in permission_ids {
            if self.find_permission(*permission_id).await?.is_none() {
                return Err(format!("Permission {permission_id} not found"));
            }
        }

        self.storage
            .execute(
                "DELETE FROM role_permissions WHERE role_id = ?1",
                vec![Value::Number(role_id.into())],
            )
            .await
            .map_err(|err| err.to_string())?;

        for permission_id in permission_ids {
            self.storage
                .execute(
                    "INSERT INTO role_permissions (role_id, permission_id) VALUES (?1, ?2)",
                    vec![
                        Value::Number(role_id.into()),
                        Value::Number((*permission_id).into()),
                    ],
                )
                .await
                .map_err(|err| err.to_string())?;
        }

        // Keep unified grants aligned with RBAC role_permissions by rebuilding
        // the derived admin.* policy grants for this role.
        self.storage
            .execute(
                r#"
                DELETE FROM role_policy_keys
                WHERE role_id = ?1
                  AND policy_key_id IN (
                      SELECT pk.id
                      FROM policy_keys pk
                      JOIN permissions p ON pk.key = ('admin.' || p.slug)
                  )
                "#,
                vec![Value::Number(role_id.into())],
            )
            .await
            .map_err(|err| err.to_string())?;

        self.storage
            .execute(
                r#"
                INSERT OR IGNORE INTO role_policy_keys (role_id, policy_key_id)
                SELECT ?1, pk.id
                FROM role_permissions rp
                JOIN permissions p ON p.id = rp.permission_id
                JOIN policy_keys pk ON pk.key = ('admin.' || p.slug)
                WHERE rp.role_id = ?1
                "#,
                vec![Value::Number(role_id.into())],
            )
            .await
            .map_err(|err| err.to_string())?;

        Ok(())
    }

    pub async fn role_has_permission(
        &self,
        role_slug: &str,
        permission_slug: &str,
    ) -> Result<bool, String> {
        let rows = self
            .storage
            .query(
                r#"
                SELECT COUNT(*) AS count
                FROM roles r
                JOIN role_permissions rp ON rp.role_id = r.id
                JOIN permissions p ON p.id = rp.permission_id
                WHERE r.slug = ?1 AND p.slug = ?2
                "#,
                vec![
                    Value::String(role_slug.trim().to_ascii_lowercase()),
                    Value::String(permission_slug.trim().to_ascii_lowercase()),
                ],
            )
            .await
            .map_err(|err| err.to_string())?;

        let count = rows
            .first()
            .and_then(|row| row.get("count"))
            .and_then(Value::as_i64)
            .unwrap_or_default();
        Ok(count > 0)
    }

    pub async fn create_permission(
        &self,
        slug: &str,
        name: &str,
        module: &str,
        description: &str,
    ) -> Result<Permission, String> {
        self.storage
            .execute(
                r#"
                INSERT INTO permissions (slug, name, module, description)
                VALUES (?1, ?2, ?3, ?4)
                "#,
                vec![
                    Value::String(slug.to_string()),
                    Value::String(name.to_string()),
                    Value::String(module.to_string()),
                    Value::String(description.to_string()),
                ],
            )
            .await
            .map_err(|err| err.to_string())?;

        let rows = self
            .storage
            .query(
                "SELECT * FROM permissions WHERE slug = ?1",
                vec![Value::String(slug.to_string())],
            )
            .await
            .map_err(|err| err.to_string())?;
        rows.into_iter()
            .next()
            .map(row_to_permission)
            .transpose()?
            .ok_or_else(|| "Permission not found after insert".to_string())
    }

    pub async fn update_permission(
        &self,
        id: i64,
        name: &str,
        module: &str,
        description: &str,
    ) -> Result<Permission, String> {
        self.storage
            .execute(
                r#"
                UPDATE permissions
                SET name = ?1, module = ?2, description = ?3, updated_at = datetime('now')
                WHERE id = ?4
                "#,
                vec![
                    Value::String(name.to_string()),
                    Value::String(module.to_string()),
                    Value::String(description.to_string()),
                    Value::Number(id.into()),
                ],
            )
            .await
            .map_err(|err| err.to_string())?;

        self.find_permission(id)
            .await?
            .ok_or_else(|| "Permission not found".to_string())
    }

    pub async fn delete_permission(&self, id: i64) -> Result<(), String> {
        let permission = self
            .find_permission(id)
            .await?
            .ok_or_else(|| "Permission not found".to_string())?;

        if permission.is_system {
            return Err("System permissions cannot be deleted".to_string());
        }

        self.storage
            .execute(
                "DELETE FROM permissions WHERE id = ?1",
                vec![Value::Number(id.into())],
            )
            .await
            .map_err(|err| err.to_string())?;
        Ok(())
    }
}

fn row_to_role_summary(
    row: std::collections::HashMap<String, Value>,
) -> Result<RoleSummary, String> {
    Ok(RoleSummary {
        id: row.get("id").and_then(Value::as_i64).unwrap_or_default(),
        slug: row
            .get("slug")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        name: row
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        description: row
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        is_system: row
            .get("is_system")
            .and_then(Value::as_i64)
            .unwrap_or_default()
            != 0,
        permission_count: row
            .get("permission_count")
            .and_then(Value::as_i64)
            .unwrap_or_default(),
        user_count: row
            .get("user_count")
            .and_then(Value::as_i64)
            .unwrap_or_default(),
    })
}

fn row_to_permission_summary(
    row: std::collections::HashMap<String, Value>,
) -> Result<PermissionSummary, String> {
    Ok(PermissionSummary {
        id: row.get("id").and_then(Value::as_i64).unwrap_or_default(),
        slug: row
            .get("slug")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        name: row
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        module: row
            .get("module")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        description: row
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        is_system: row
            .get("is_system")
            .and_then(Value::as_i64)
            .unwrap_or_default()
            != 0,
        role_count: row
            .get("role_count")
            .and_then(Value::as_i64)
            .unwrap_or_default(),
    })
}

fn row_to_role_permission_assignment(
    row: std::collections::HashMap<String, Value>,
) -> Result<RolePermissionAssignment, String> {
    Ok(RolePermissionAssignment {
        id: row.get("id").and_then(Value::as_i64).unwrap_or_default(),
        slug: row
            .get("slug")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        name: row
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        module: row
            .get("module")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        description: row
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        is_system: row
            .get("is_system")
            .and_then(Value::as_i64)
            .unwrap_or_default()
            != 0,
        assigned: row
            .get("assigned")
            .and_then(Value::as_i64)
            .unwrap_or_default()
            != 0,
    })
}

fn row_to_role(row: std::collections::HashMap<String, Value>) -> Result<Role, String> {
    Ok(Role {
        id: row.get("id").and_then(Value::as_i64).unwrap_or_default(),
        slug: row
            .get("slug")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        name: row
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        description: row
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        is_system: row
            .get("is_system")
            .and_then(Value::as_i64)
            .unwrap_or_default()
            != 0,
        created_at: row
            .get("created_at")
            .and_then(Value::as_str)
            .map(parse_sqlite_datetime)
            .unwrap_or_default(),
        updated_at: row
            .get("updated_at")
            .and_then(Value::as_str)
            .map(parse_sqlite_datetime)
            .unwrap_or_default(),
    })
}

fn row_to_permission(row: std::collections::HashMap<String, Value>) -> Result<Permission, String> {
    Ok(Permission {
        id: row.get("id").and_then(Value::as_i64).unwrap_or_default(),
        slug: row
            .get("slug")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        name: row
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        module: row
            .get("module")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        description: row
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        is_system: row
            .get("is_system")
            .and_then(Value::as_i64)
            .unwrap_or_default()
            != 0,
        created_at: row
            .get("created_at")
            .and_then(Value::as_str)
            .map(parse_sqlite_datetime)
            .unwrap_or_default(),
        updated_at: row
            .get("updated_at")
            .and_then(Value::as_str)
            .map(parse_sqlite_datetime)
            .unwrap_or_default(),
    })
}

fn parse_sqlite_datetime(input: &str) -> DateTime<Utc> {
    chrono::NaiveDateTime::parse_from_str(input, "%Y-%m-%d %H:%M:%S")
        .ok()
        .map(|dt| DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::RbacRepository;
    use crate::storage::sqlite::SqliteStorage;
    use crate::storage::Storage;
    use serde_json::Value;
    use std::sync::Arc;

    async fn repo_with_schema() -> (RbacRepository, Arc<dyn Storage>) {
        let sqlite = Arc::new(
            SqliteStorage::new_in_memory()
                .await
                .expect("sqlite setup should work"),
        );
        sqlite
            .run_migrations(include_str!("../../../../migrations/001_init.sql"))
            .await
            .expect("migration 001 should apply");
        sqlite
            .run_migrations(include_str!("../../../../migrations/003_rbac.sql"))
            .await
            .expect("migration 003 should apply");
        sqlite
            .run_migrations(include_str!(
                "../../../../migrations/006_unified_policy_v2.sql"
            ))
            .await
            .expect("migration 006 should apply");

        let storage: Arc<dyn Storage> = sqlite;
        (RbacRepository::new(Arc::clone(&storage)), storage)
    }

    #[tokio::test]
    async fn replace_role_permissions_syncs_role_policy_keys() {
        let (repo, storage) = repo_with_schema().await;

        let role_rows = storage
            .query("SELECT id FROM roles WHERE slug = 'editor'", vec![])
            .await
            .expect("role query should succeed");
        let role_id = role_rows
            .first()
            .and_then(|row| row.get("id"))
            .and_then(Value::as_i64)
            .expect("editor role id should exist");

        let permission_rows = storage
            .query(
                "SELECT id FROM permissions WHERE slug = 'users.view'",
                vec![],
            )
            .await
            .expect("permission query should succeed");
        let users_view_permission_id = permission_rows
            .first()
            .and_then(|row| row.get("id"))
            .and_then(Value::as_i64)
            .expect("users.view permission id should exist");

        repo.replace_role_permissions(role_id, &[users_view_permission_id])
            .await
            .expect("role permissions should update");

        let grant_rows = storage
            .query(
                r#"
                SELECT pk.key
                FROM role_policy_keys rpk
                JOIN policy_keys pk ON pk.id = rpk.policy_key_id
                JOIN roles r ON r.id = rpk.role_id
                WHERE r.slug = 'editor'
                ORDER BY pk.key ASC
                "#,
                vec![],
            )
            .await
            .expect("policy grant query should succeed");
        let policy_keys = grant_rows
            .into_iter()
            .filter_map(|row| row.get("key").and_then(Value::as_str).map(str::to_string))
            .collect::<Vec<_>>();

        assert!(
            policy_keys.iter().any(|key| key == "admin.users.view"),
            "admin.users.view should be granted after role permission update"
        );
        assert!(
            !policy_keys.iter().any(|key| key == "admin.dashboard.view"),
            "stale admin policy grants should be removed after role permission update"
        );
        assert!(
            policy_keys.iter().any(|key| key == "api.users.read"),
            "non-admin seeded grants should remain intact"
        );
    }
}
