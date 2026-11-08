use axum::extract::{Path, Query, State};
use axum::Json;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::postgres::Postgres;
use sqlx::types::Json as SqlJson;
use sqlx::QueryBuilder;
use uuid::Uuid;

use crate::auth::ApiAuth;
use crate::error::AppError;
use crate::AppState;

#[derive(Debug, Deserialize, Default)]
pub struct ListParams {
    pub q: Option<String>,
    pub domain: Option<String>,
    pub os: Option<String>,
    pub maintenance: Option<String>,     // "true" | "false" | "any"
    pub live_state: Option<String>,      // "online" | "offline" | "maintenance" | "never"
    pub sort: Option<String>,
    pub order: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub async fn list(
    State(state): State<AppState>,
    _auth: ApiAuth,
    Query(p): Query<ListParams>,
) -> Result<Json<Value>, AppError> {
    let limit = p.limit.unwrap_or(500).clamp(1, 2000);
    let offset = p.offset.unwrap_or(0).max(0);

    let mut qb: QueryBuilder<Postgres> =
        QueryBuilder::new("SELECT to_jsonb(s) FROM systems_current s WHERE 1=1");

    if let Some(q) = &p.q {
        let like = format!("%{}%", q);
        qb.push(" AND (s.computer_name ILIKE ");
        qb.push_bind(like.clone());
        qb.push(" OR s.fqdn ILIKE ");
        qb.push_bind(like.clone());
        qb.push(" OR array_to_string(s.ip_addresses, ',') ILIKE ");
        qb.push_bind(like);
        qb.push(")");
    }
    if let Some(d) = &p.domain { qb.push(" AND s.domain = "); qb.push_bind(d.clone()); }
    if let Some(o) = &p.os     { qb.push(" AND s.os_name ILIKE "); qb.push_bind(format!("%{}%", o)); }
    if let Some(m) = &p.maintenance {
        if m == "true"  { qb.push(" AND s.maintenance = TRUE"); }
        if m == "false" { qb.push(" AND s.maintenance = FALSE"); }
    }
    if let Some(ls) = &p.live_state {
        if ls != "any" { qb.push(" AND s.live_state = "); qb.push_bind(ls.clone()); }
    }

    let sort_col = match p.sort.as_deref() {
        Some("last_seen") => "s.last_seen",
        Some("disk_free") => "s.system_drive_free",
        Some("os")        => "s.os_name",
        _ => "s.computer_name",
    };
    let order = if p.order.as_deref() == Some("desc") { "DESC" } else { "ASC" };
    qb.push(format!(" ORDER BY {} {}", sort_col, order));
    qb.push(" LIMIT ");   qb.push_bind(limit);
    qb.push(" OFFSET ");  qb.push_bind(offset);

    let rows: Vec<SqlJson<Value>> = qb.build_query_scalar().fetch_all(&state.pool).await?;
    let items: Vec<Value> = rows.into_iter().map(|j| j.0).collect();

    Ok(Json(json!({
        "items": items,
        "count": items.len(),
        "limit": limit,
        "offset": offset,
    })))
}

pub async fn get_one(
    State(state): State<AppState>,
    _auth: ApiAuth,
    Path(agent_id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    let row: Option<SqlJson<Value>> = sqlx::query_scalar(
        "SELECT to_jsonb(s) FROM systems_current s WHERE agent_id = $1"
    )
    .bind(agent_id)
    .fetch_optional(&state.pool)
    .await?;
    row.map(|SqlJson(v)| Json(v)).ok_or(AppError::NotFound)
}

pub async fn force_refresh(
    State(state): State<AppState>,
    _auth: ApiAuth,
    Path(agent_id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    let res = sqlx::query("UPDATE systems SET force_refresh = true WHERE agent_id = $1")
        .bind(agent_id)
        .execute(&state.pool)
        .await?;
    if res.rows_affected() == 0 { return Err(AppError::NotFound); }
    Ok(Json(json!({ "ok": true })))
}

#[derive(Debug, Deserialize)]
pub struct MaintenanceReq { pub on: bool }

pub async fn set_maintenance(
    State(state): State<AppState>,
    _auth: ApiAuth,
    Path(agent_id): Path<Uuid>,
    Json(req): Json<MaintenanceReq>,
) -> Result<Json<Value>, AppError> {
    let res = if req.on {
        sqlx::query("UPDATE systems SET maintenance = true, maintenance_since = now() WHERE agent_id = $1")
            .bind(agent_id).execute(&state.pool).await?
    } else {
        sqlx::query("UPDATE systems SET maintenance = false, maintenance_since = NULL WHERE agent_id = $1")
            .bind(agent_id).execute(&state.pool).await?
    };
    if res.rows_affected() == 0 { return Err(AppError::NotFound); }
    Ok(Json(json!({ "ok": true, "maintenance": req.on })))
}

#[derive(Debug, Deserialize)]
pub struct AddNoteReq { pub body: String, pub author: Option<String> }

pub async fn add_note(
    State(state): State<AppState>,
    _auth: ApiAuth,
    Path(agent_id): Path<Uuid>,
    Json(req): Json<AddNoteReq>,
) -> Result<Json<Value>, AppError> {
    if req.body.trim().is_empty() {
        return Err(AppError::BadRequest("empty note".into()));
    }
    let (id,): (i64,) = sqlx::query_as(
        "INSERT INTO system_notes (agent_id, body, author) VALUES ($1, $2, $3) RETURNING id"
    )
    .bind(agent_id).bind(&req.body).bind(req.author.as_deref())
    .fetch_one(&state.pool).await?;
    Ok(Json(json!({ "id": id })))
}

pub async fn list_notes(
    State(state): State<AppState>,
    _auth: ApiAuth,
    Path(agent_id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    let rows: Vec<(i64, String, Option<String>, DateTime<Utc>)> = sqlx::query_as(
        "SELECT id, body, author, created_at FROM system_notes WHERE agent_id = $1 ORDER BY created_at DESC"
    )
    .bind(agent_id).fetch_all(&state.pool).await?;
    let items: Vec<Value> = rows.into_iter().map(|(id, body, author, created_at)| {
        json!({ "id": id, "body": body, "author": author, "created_at": created_at })
    }).collect();
    Ok(Json(json!({ "items": items })))
}

pub async fn stats(
    State(state): State<AppState>,
    _auth: ApiAuth,
) -> Result<Json<Value>, AppError> {
    let (online, offline, maintenance): (i64, i64, i64) = sqlx::query_as(
        "SELECT
           COUNT(*) FILTER (WHERE live_state = 'online')::bigint,
           COUNT(*) FILTER (WHERE live_state = 'offline')::bigint,
           COUNT(*) FILTER (WHERE maintenance = true)::bigint
         FROM systems_current"
    ).fetch_one(&state.pool).await?;
    Ok(Json(json!({
        "online": online,
        "offline": offline,
        "maintenance": maintenance,
        "total": online + offline,
    })))
}

pub async fn get_settings(
    State(state): State<AppState>,
    _auth: ApiAuth,
) -> Result<Json<Value>, AppError> {
    let rows: Vec<(String, Value)> = sqlx::query_as("SELECT key, value FROM settings")
        .fetch_all(&state.pool).await?;
    let mut out = serde_json::Map::new();
    for (k, v) in rows { out.insert(k, v); }
    Ok(Json(Value::Object(out)))
}

pub async fn put_settings(
    State(state): State<AppState>,
    _auth: ApiAuth,
    Json(body): Json<Value>,
) -> Result<Json<Value>, AppError> {
    let obj = body.as_object()
        .ok_or(AppError::BadRequest("expected JSON object".into()))?;
    for (k, v) in obj {
        sqlx::query(
            "INSERT INTO settings (key, value, updated_at) VALUES ($1, $2, now())
             ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, updated_at = now()"
        )
        .bind(k).bind(SqlJson(v))
        .execute(&state.pool).await?;
    }
    Ok(Json(json!({ "ok": true })))
}


use crate::auth::{generate_token, hash_token};

#[derive(Debug, Deserialize)]
pub struct CreateKeyReq {
    pub label: String,
}

pub async fn create_api_key(
    State(state): State<AppState>,
    _auth: ApiAuth,
    Json(req): Json<CreateKeyReq>,
) -> Result<Json<Value>, AppError> {
    if req.label.trim().is_empty() {
        return Err(AppError::BadRequest("label required".into()));
    }
    let key = generate_token();
    let hash = hash_token(&key);
    sqlx::query("INSERT INTO api_keys (key_hash, label) VALUES ($1, $2)")
        .bind(&hash)
        .bind(&req.label)
        .execute(&state.pool)
        .await?;
    tracing::info!(label = %req.label, "new api key issued");
    Ok(Json(json!({
        "key": key,
        "label": req.label,
        "note": "Save this now — it will not be shown again.",
    })))
}

pub async fn list_api_keys(
    State(state): State<AppState>,
    _auth: ApiAuth,
) -> Result<Json<Value>, AppError> {
    let rows: Vec<(String, chrono::DateTime<chrono::Utc>, Option<chrono::DateTime<chrono::Utc>>, Option<chrono::DateTime<chrono::Utc>>)> =
        sqlx::query_as(
            "SELECT label, created_at, last_used, revoked_at FROM api_keys ORDER BY created_at DESC"
        )
        .fetch_all(&state.pool)
        .await?;
    let items: Vec<Value> = rows
        .into_iter()
        .map(|(label, created_at, last_used, revoked_at)| {
            json!({
                "label": label,
                "created_at": created_at,
                "last_used": last_used,
                "revoked": revoked_at.is_some(),
            })
        })
        .collect();
    Ok(Json(json!({ "items": items })))
}

// ---------- agent registry ----------

pub async fn list_agents(
    State(state): State<AppState>,
    _auth: ApiAuth,
) -> Result<Json<Value>, AppError> {
    let rows: Vec<(
        uuid::Uuid,                // agent_id
        Option<String>,            // computer_name (from agents row)
        chrono::DateTime<chrono::Utc>,  // enrolled_at
        Option<chrono::DateTime<chrono::Utc>>, // last_seen
        Option<chrono::DateTime<chrono::Utc>>, // revoked_at
    )> = sqlx::query_as(
        "SELECT agent_id, computer_name, enrolled_at, last_seen, revoked_at
         FROM agents ORDER BY enrolled_at DESC"
    )
    .fetch_all(&state.pool)
    .await?;

    let items: Vec<Value> = rows.into_iter().map(|(id, name, enrolled, last_seen, revoked)| {
        json!({
            "agent_id": id,
            "computer_name": name,
            "enrolled_at": enrolled,
            "last_seen": last_seen,
            "revoked": revoked.is_some(),
            "revoked_at": revoked,
        })
    }).collect();
    Ok(Json(json!({ "items": items })))
}

pub async fn revoke_agent(
    State(state): State<AppState>,
    _auth: ApiAuth,
    axum::extract::Path(agent_id): axum::extract::Path<uuid::Uuid>,
) -> Result<Json<Value>, AppError> {
    let res = sqlx::query(
        "UPDATE agents SET revoked_at = now() WHERE agent_id = $1 AND revoked_at IS NULL"
    )
    .bind(agent_id)
    .execute(&state.pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    tracing::warn!(%agent_id, "agent token revoked");
    Ok(Json(json!({ "ok": true })))
}

pub async fn reinstate_agent(
    State(state): State<AppState>,
    _auth: ApiAuth,
    axum::extract::Path(agent_id): axum::extract::Path<uuid::Uuid>,
) -> Result<Json<Value>, AppError> {
    let res = sqlx::query(
        "UPDATE agents SET revoked_at = NULL WHERE agent_id = $1"
    )
    .bind(agent_id)
    .execute(&state.pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(Json(json!({ "ok": true })))
}