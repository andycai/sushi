use crate::storage::{Row, Storage, StorageError};
use serde_json::Value;
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbPermission {
    ReadOnly,
    Write,
    Admin,
}

#[derive(Error, Debug)]
pub enum DbGatewayError {
    #[error("permission denied: {0}")]
    PermissionDenied(String),

    #[error(transparent)]
    Storage(#[from] StorageError),
}

#[derive(Clone)]
pub struct DbGateway {
    storage: Arc<dyn Storage>,
    permission: DbPermission,
}

impl DbGateway {
    pub fn new(storage: Arc<dyn Storage>, permission: DbPermission) -> Self {
        Self {
            storage,
            permission,
        }
    }

    pub(crate) fn with_permission(&self, permission: DbPermission) -> Self {
        Self {
            storage: Arc::clone(&self.storage),
            permission,
        }
    }

    pub async fn query(&self, sql: &str, params: Vec<Value>) -> Result<Vec<Row>, DbGatewayError> {
        self.check_permission(sql)?;
        Storage::query(&*self.storage, sql, params)
            .await
            .map_err(DbGatewayError::from)
    }

    pub async fn execute(&self, sql: &str, params: Vec<Value>) -> Result<(), DbGatewayError> {
        self.check_permission(sql)?;
        Storage::execute(&*self.storage, sql, params)
            .await
            .map_err(DbGatewayError::from)
    }

    fn check_permission(&self, sql: &str) -> Result<(), DbGatewayError> {
        if has_multiple_statements(sql) {
            return Err(DbGatewayError::PermissionDenied(
                "multiple SQL statements are not allowed".to_string(),
            ));
        }

        let operation = classify_sql(sql);
        let allowed = match self.permission {
            DbPermission::Admin => true,
            DbPermission::Write => operation != SqlOperation::Admin,
            DbPermission::ReadOnly => operation == SqlOperation::Read,
        };

        if allowed {
            Ok(())
        } else {
            Err(DbGatewayError::PermissionDenied(format!(
                "{permission:?} does not allow {operation:?} SQL",
                permission = self.permission,
                operation = operation,
            )))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SqlOperation {
    Read,
    Write,
    Admin,
}

fn classify_sql(sql: &str) -> SqlOperation {
    let statement = first_statement_keyword(sql);
    match statement.as_deref() {
        Some(keyword) => classify_keyword(keyword).unwrap_or(SqlOperation::Admin),
        None => SqlOperation::Admin,
    }
}

fn has_multiple_statements(sql: &str) -> bool {
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut in_line_comment = false;
    let mut in_block_comment = false;
    let mut saw_semicolon = false;
    let mut chars = sql.chars().peekable();

    while let Some(ch) = chars.next() {
        if in_line_comment {
            if ch == '\n' {
                in_line_comment = false;
            }
            continue;
        }

        if in_block_comment {
            if ch == '*' && chars.peek() == Some(&'/') {
                chars.next();
                in_block_comment = false;
            }
            continue;
        }

        if in_single_quote {
            if ch == '\'' {
                if chars.peek() == Some(&'\'') {
                    chars.next();
                } else {
                    in_single_quote = false;
                }
            }
            continue;
        }

        if in_double_quote {
            if ch == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                } else {
                    in_double_quote = false;
                }
            }
            continue;
        }

        if ch == '-' && chars.peek() == Some(&'-') {
            chars.next();
            in_line_comment = true;
            continue;
        }

        if ch == '/' && chars.peek() == Some(&'*') {
            chars.next();
            in_block_comment = true;
            continue;
        }

        if ch == '\'' {
            in_single_quote = true;
            continue;
        }

        if ch == '"' {
            in_double_quote = true;
            continue;
        }

        if ch == ';' {
            if saw_semicolon {
                return true;
            }
            saw_semicolon = true;
            continue;
        }

        if saw_semicolon {
            if ch.is_whitespace() {
                continue;
            }
            if ch == '-' && chars.peek() == Some(&'-') {
                chars.next();
                in_line_comment = true;
                continue;
            }
            if ch == '/' && chars.peek() == Some(&'*') {
                chars.next();
                in_block_comment = true;
                continue;
            }
            return true;
        }
    }

    false
}

fn classify_keyword(keyword: &str) -> Option<SqlOperation> {
    match keyword {
        "select" | "values" | "explain" => Some(SqlOperation::Read),
        "insert" | "update" | "delete" | "replace" | "upsert" => Some(SqlOperation::Write),
        "create" | "drop" | "alter" | "truncate" | "pragma" | "attach" | "detach" | "reindex"
        | "vacuum" | "begin" | "commit" | "rollback" | "analyze" => Some(SqlOperation::Admin),
        _ => None,
    }
}

fn first_statement_keyword(sql: &str) -> Option<String> {
    let mut token = String::new();
    let mut found = None;
    let mut paren_depth = 0usize;
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut chars = strip_leading_comments(sql).chars().peekable();

    while let Some(ch) = chars.next() {
        if in_single_quote {
            if ch == '\'' {
                in_single_quote = false;
            }
            continue;
        }

        if in_double_quote {
            if ch == '"' {
                in_double_quote = false;
            }
            continue;
        }

        match ch {
            '\'' => {
                in_single_quote = true;
                token.clear();
                continue;
            }
            '"' => {
                in_double_quote = true;
                token.clear();
                continue;
            }
            '(' => {
                paren_depth = paren_depth.saturating_add(1);
                token.clear();
                continue;
            }
            ')' => {
                if paren_depth > 0 {
                    paren_depth -= 1;
                }
                token.clear();
                continue;
            }
            _ => {}
        }

        if paren_depth > 0 {
            continue;
        }

        if is_token_char(ch) {
            token.push(ch);
        } else if !token.is_empty() {
            let lowered = token.to_ascii_lowercase();
            if classify_keyword(&lowered).is_some() {
                found = Some(lowered);
                break;
            }
            token.clear();
        }
    }

    if found.is_none() && !token.is_empty() {
        let lowered = token.to_ascii_lowercase();
        if classify_keyword(&lowered).is_some() {
            found = Some(lowered);
        }
    }

    found
}

fn strip_leading_comments(sql: &str) -> &str {
    let mut remaining = sql;
    loop {
        let trimmed = remaining.trim_start();
        if trimmed.starts_with("--") {
            if let Some(pos) = trimmed.find('\n') {
                remaining = &trimmed[pos + 1..];
                continue;
            }
            return "";
        }
        if trimmed.starts_with("/*") {
            if let Some(pos) = trimmed.find("*/") {
                remaining = &trimmed[pos + 2..];
                continue;
            }
            return "";
        }
        return trimmed;
    }
}

fn is_token_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_skips_cte_names() {
        let sql = "WITH recent AS (SELECT * FROM t) SELECT * FROM recent";
        assert_eq!(classify_sql(sql), SqlOperation::Read);
    }

    #[test]
    fn classify_insert_with_cte() {
        let sql = "WITH latest AS (SELECT * FROM t) INSERT INTO t SELECT * FROM latest";
        assert_eq!(classify_sql(sql), SqlOperation::Write);
    }
}
