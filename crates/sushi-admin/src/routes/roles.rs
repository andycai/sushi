use axum::extract::{Form, Path, State};
use axum::http::{header::HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::de::{Deserializer, SeqAccess, Visitor};
use serde::Deserialize;
use std::sync::Arc;
use sushi_core::auth::rbac::RbacRepository;
use sushi_core::context::SushiContext;
use sushi_core::storage::Storage;

pub async fn roles_page(State(ctx): State<SushiContext>) -> impl IntoResponse {
    crate::render::render_template(&ctx, "admin/roles.html").await
}

pub async fn roles_table_partial(State(ctx): State<SushiContext>) -> impl IntoResponse {
    render_roles_rows(&ctx).await
}

#[derive(Debug, Deserialize)]
pub struct CreateRoleForm {
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
}

pub async fn roles_create_partial(
    State(ctx): State<SushiContext>,
    Form(form): Form<CreateRoleForm>,
) -> impl IntoResponse {
    if let Err(message) = validate_create_role_form(&form) {
        return flash_response(&ctx, StatusCode::BAD_REQUEST, "error", &message).await;
    }

    let repo = RbacRepository::new(ctx.db.clone() as Arc<dyn Storage>);
    match repo
        .create_role(
            &form.slug.trim().to_ascii_lowercase(),
            form.name.trim(),
            form.description.as_deref().unwrap_or_default().trim(),
        )
        .await
    {
        Ok(_) => {
            flash_response_with_trigger(
                &ctx,
                StatusCode::OK,
                "success",
                "Role created.",
                r#"{"roles:refresh":true,"roles:close-role-drawer":true}"#,
            )
            .await
        }
        Err(err) => flash_response(&ctx, StatusCode::BAD_REQUEST, "error", &err).await,
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdateRoleForm {
    pub name: String,
    pub description: Option<String>,
}

pub async fn roles_update_partial(
    State(ctx): State<SushiContext>,
    Path(id): Path<i64>,
    Form(form): Form<UpdateRoleForm>,
) -> impl IntoResponse {
    if let Err(message) = validate_update_role_form(&form) {
        return flash_response(&ctx, StatusCode::BAD_REQUEST, "error", &message).await;
    }

    let repo = RbacRepository::new(ctx.db.clone() as Arc<dyn Storage>);
    match repo
        .update_role(
            id,
            form.name.trim(),
            form.description.as_deref().unwrap_or_default().trim(),
        )
        .await
    {
        Ok(_) => {
            flash_response_with_trigger(
                &ctx,
                StatusCode::OK,
                "success",
                "Role updated.",
                r#"{"roles:refresh":true,"roles:close-role-drawer":true}"#,
            )
            .await
        }
        Err(err) => flash_response(&ctx, StatusCode::BAD_REQUEST, "error", &err).await,
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdateRolePermissionsForm {
    #[serde(
        default,
        alias = "permission_ids[]",
        deserialize_with = "deserialize_permission_ids"
    )]
    pub permission_ids: Vec<i64>,
}

fn deserialize_permission_ids<'de, D>(deserializer: D) -> Result<Vec<i64>, D::Error>
where
    D: Deserializer<'de>,
{
    struct PermissionIdsVisitor;

    impl<'de> Visitor<'de> for PermissionIdsVisitor {
        type Value = Vec<i64>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a permission id or a list of permission ids")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            parse_permission_id(value)
                .map(|id| vec![id])
                .map_err(E::custom)
        }

        fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            self.visit_str(&value)
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            let mut ids = Vec::new();
            while let Some(value) = seq.next_element::<String>()? {
                ids.push(parse_permission_id(&value).map_err(serde::de::Error::custom)?);
            }
            Ok(ids)
        }

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(Vec::new())
        }

        fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(Vec::new())
        }
    }

    deserializer.deserialize_any(PermissionIdsVisitor)
}

fn parse_permission_id(input: &str) -> Result<i64, String> {
    let value = input.trim();
    value
        .parse::<i64>()
        .map_err(|_| format!("invalid permission id: {value}"))
}

pub async fn role_permissions_form_partial(
    State(ctx): State<SushiContext>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let repo = RbacRepository::new(ctx.db.clone() as Arc<dyn Storage>);
    let role = match repo.find_role(id).await {
        Ok(Some(role)) => role,
        Ok(None) => {
            return flash_response(&ctx, StatusCode::NOT_FOUND, "error", "Role not found").await;
        }
        Err(err) => {
            return flash_response(&ctx, StatusCode::INTERNAL_SERVER_ERROR, "error", &err).await;
        }
    };

    let assignments = match repo.list_permissions_for_role(id).await {
        Ok(items) => items,
        Err(err) => {
            return flash_response(&ctx, StatusCode::INTERNAL_SERVER_ERROR, "error", &err).await;
        }
    };

    crate::render::render_template_with_context(
        &ctx,
        "admin/partials/role_permissions_form.html",
        serde_json::json!({
            "role": {
                "id": role.id,
                "slug": role.slug,
                "name": role.name,
                "description": role.description,
                "is_system": role.is_system,
            },
            "permissions": assignments,
        }),
    )
    .await
}

pub async fn role_permissions_update_partial(
    State(ctx): State<SushiContext>,
    Path(id): Path<i64>,
    Form(form): Form<UpdateRolePermissionsForm>,
) -> impl IntoResponse {
    let repo = RbacRepository::new(ctx.db.clone() as Arc<dyn Storage>);

    // De-duplicate checkbox values while preserving deterministic order.
    let mut deduped = Vec::new();
    for permission_id in form.permission_ids {
        if !deduped.contains(&permission_id) {
            deduped.push(permission_id);
        }
    }

    match repo.replace_role_permissions(id, &deduped).await {
        Ok(_) => {
            if let Err(err) = ctx.refresh_authorizer_snapshot().await {
                return flash_response(
                    &ctx,
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "error",
                    &format!("Role permissions updated but policy refresh failed: {err}"),
                )
                .await;
            }
            flash_response_with_trigger(
                &ctx,
                StatusCode::OK,
                "success",
                "Role permissions updated.",
                r#"{"roles:refresh":true,"roles:close-permissions-modal":true}"#,
            )
            .await
        }
        Err(err) => flash_response(&ctx, StatusCode::BAD_REQUEST, "error", &err).await,
    }
}

pub async fn roles_delete_partial(
    State(ctx): State<SushiContext>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let repo = RbacRepository::new(ctx.db.clone() as Arc<dyn Storage>);
    match repo.delete_role(id).await {
        Ok(_) => {
            flash_response_with_trigger(
                &ctx,
                StatusCode::OK,
                "success",
                "Role deleted.",
                "roles:refresh",
            )
            .await
        }
        Err(err) => flash_response(&ctx, StatusCode::BAD_REQUEST, "error", &err).await,
    }
}

fn validate_create_role_form(form: &CreateRoleForm) -> Result<(), String> {
    let slug = form.slug.trim();
    if slug.len() < 3 || slug.len() > 40 {
        return Err("Role key must be between 3 and 40 characters".to_string());
    }
    if !slug
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '-')
    {
        return Err(
            "Role key can only include lowercase letters, numbers, dashes, and underscores"
                .to_string(),
        );
    }

    validate_role_name_and_description(&form.name, form.description.as_deref())
}

fn validate_update_role_form(form: &UpdateRoleForm) -> Result<(), String> {
    validate_role_name_and_description(&form.name, form.description.as_deref())
}

fn validate_role_name_and_description(name: &str, description: Option<&str>) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Role name is required".to_string());
    }
    if name.len() > 80 {
        return Err("Role name must be 80 characters or fewer".to_string());
    }

    if let Some(description) = description {
        if description.trim().len() > 280 {
            return Err("Role description must be 280 characters or fewer".to_string());
        }
    }

    Ok(())
}

async fn render_roles_rows(ctx: &SushiContext) -> Response {
    let repo = RbacRepository::new(ctx.db.clone() as Arc<dyn Storage>);
    let roles = match repo.list_roles().await {
        Ok(roles) => roles,
        Err(err) => {
            return flash_response(ctx, StatusCode::INTERNAL_SERVER_ERROR, "error", &err).await;
        }
    };

    crate::render::render_template_with_context(
        ctx,
        "admin/partials/roles_rows.html",
        serde_json::json!({
            "roles": roles,
        }),
    )
    .await
}

async fn flash_response(
    ctx: &SushiContext,
    status: StatusCode,
    level: &str,
    message: &str,
) -> Response {
    let mut response = crate::render::render_template_with_context(
        ctx,
        "admin/partials/flash.html",
        serde_json::json!({
            "level": level,
            "message": message,
        }),
    )
    .await;
    *response.status_mut() = status;
    response
}

async fn flash_response_with_trigger(
    ctx: &SushiContext,
    status: StatusCode,
    level: &str,
    message: &str,
    trigger: &str,
) -> Response {
    let mut response = flash_response(ctx, status, level, message).await;
    if let Ok(value) = HeaderValue::from_str(trigger) {
        response
            .headers_mut()
            .insert(HeaderName::from_static("hx-trigger"), value);
    }
    response
}
