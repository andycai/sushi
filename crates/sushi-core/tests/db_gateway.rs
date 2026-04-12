use sushi_core::db::{DbGateway, DbGatewayError, DbPermission};
use sushi_core::storage::sqlite::SqliteStorage;
use sushi_core::storage::Storage;
use std::sync::Arc;

#[tokio::test]
async fn readonly_rejects_insert() {
    let storage = SqliteStorage::new_in_memory().await.unwrap();
    Storage::execute(
        &storage,
        "CREATE TABLE test_items (id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
        vec![],
    )
    .await
    .unwrap();

    let gateway = DbGateway::new(Arc::new(storage), DbPermission::ReadOnly);
    let result = gateway
        .execute(
            "INSERT INTO test_items (name) VALUES (?1)",
            vec![serde_json::Value::String("nope".to_string())],
        )
        .await;

    assert!(matches!(result, Err(DbGatewayError::PermissionDenied(_))));
}

#[tokio::test]
async fn write_allows_insert_and_query() {
    let storage = SqliteStorage::new_in_memory().await.unwrap();
    Storage::execute(
        &storage,
        "CREATE TABLE test_items (id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
        vec![],
    )
    .await
    .unwrap();

    let gateway = DbGateway::new(Arc::new(storage), DbPermission::Write);
    gateway
        .execute(
            "INSERT INTO test_items (name) VALUES (?1)",
            vec![serde_json::Value::String("ok".to_string())],
        )
        .await
        .unwrap();

    let rows = gateway
        .query("SELECT name FROM test_items", vec![])
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get("name").and_then(|v| v.as_str()), Some("ok"));
}

#[tokio::test]
async fn admin_allows_ddl() {
    let storage = SqliteStorage::new_in_memory().await.unwrap();
    let gateway = DbGateway::new(Arc::new(storage), DbPermission::Admin);

    gateway
        .execute(
            "CREATE TABLE admin_items (id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
            vec![],
        )
        .await
        .unwrap();

    gateway
        .execute(
            "INSERT INTO admin_items (name) VALUES (?1)",
            vec![serde_json::Value::String("admin".to_string())],
        )
        .await
        .unwrap();

    let rows = gateway
        .query("SELECT name FROM admin_items", vec![])
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
}

#[tokio::test]
async fn rejects_multiple_statements() {
    let storage = SqliteStorage::new_in_memory().await.unwrap();
    let gateway = DbGateway::new(Arc::new(storage), DbPermission::Admin);

    let result = gateway
        .execute(
            "CREATE TABLE multi_items (id INTEGER); INSERT INTO multi_items (id) VALUES (1)",
            vec![],
        )
        .await;

    assert!(matches!(result, Err(DbGatewayError::PermissionDenied(_))));
}
