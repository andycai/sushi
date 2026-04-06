use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

use sushi_core::kv::KvStore;
use sushi_core::storage::sqlite::SqliteStorage;

#[derive(Clone)]
pub struct KvRouteState {
    pub storage: Arc<SqliteStorage>,
}

pub fn kv_routes(state: KvRouteState) -> Router {
    Router::new()
        .route("/", get(list_kv).post(set_kv))
        .route("/{key}", get(get_kv).put(update_kv).delete(delete_kv))
        .with_state(state)
}

async fn list_kv(State(state): State<KvRouteState>) -> impl IntoResponse {
    let kv = KvStore::new(state.storage);
    match kv.list().await {
        Ok(items) => {
            let response: Vec<Value> = items
                .into_iter()
                .map(|(k, v)| json!({ "key": k, "value": v }))
                .collect();
            (StatusCode::OK, Json(json!(response))).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
pub struct SetKvRequest {
    pub key: String,
    pub value: String,
}

async fn set_kv(
    State(state): State<KvRouteState>,
    Json(req): Json<SetKvRequest>,
) -> impl IntoResponse {
    let kv = KvStore::new(state.storage);
    match kv.set(&req.key, &req.value).await {
        Ok(()) => (StatusCode::CREATED, Json(json!({ "key": req.key, "value": req.value })))
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn get_kv(
    State(state): State<KvRouteState>,
    Path(key): Path<String>,
) -> impl IntoResponse {
    let kv = KvStore::new(state.storage);
    match kv.get(&key).await {
        Ok(Some(value)) => {
            (StatusCode::OK, Json(json!({ "key": key, "value": value }))).into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "key not found" })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
pub struct UpdateKvRequest {
    pub value: String,
}

async fn update_kv(
    State(state): State<KvRouteState>,
    Path(key): Path<String>,
    Json(req): Json<UpdateKvRequest>,
) -> impl IntoResponse {
    let kv = KvStore::new(state.storage);
    match kv.set(&key, &req.value).await {
        Ok(()) => (StatusCode::OK, Json(json!({ "key": key, "value": req.value }))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn delete_kv(
    State(state): State<KvRouteState>,
    Path(key): Path<String>,
) -> impl IntoResponse {
    let kv = KvStore::new(state.storage);
    match kv.delete(&key).await {
        Ok(()) => (StatusCode::NO_CONTENT, Json(json!(null))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}
