use axum::extract::State;
use axum::http::header::AUTHORIZATION;
use axum::http::HeaderMap;
use axum::Json;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::types::Json as SqlJson;
use uuid::Uuid;

use crate::auth::{generate_token, hash_token, AgentAuth};
use crate::broadcast::SystemEvent;
use crate::error::AppError;
use crate::AppState;

// ---------- enroll ----------

#[derive(Debug, Deserialize)]
pub struct EnrollReq {
    pub computer_name: Option<String>,
}

pub async fn enroll(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<EnrollReq>,
) -> Result<Json<Value>, AppError> {
    let bearer = headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(AppError::Unauthorized("missing enrollment secret"))?;

    let stored: (Value,) =
        sqlx::query_as("SELECT value FROM settings WHERE key = 'enrollment_secret'")
            .fetch_one(&state.pool)
            .await?;
    let stored_secret = stored.0.as_str().unwrap_or("").to_string();
    if stored_secret.is_empty() {
        return Err(AppError::Unauthorized("enrollment not configured"));
    }
    if bearer != stored_secret {
        return Err(AppError::Unauthorized("invalid enrollment secret"));
    }

    let agent_id = Uuid::new_v4();
    let token = generate_token();
    let token_hash = hash_token(&token);

    sqlx::query("INSERT INTO agents (agent_id, token_hash, computer_name) VALUES ($1, $2, $3)")
        .bind(agent_id)
        .bind(&token_hash)
        .bind(req.computer_name.as_deref())
        .execute(&state.pool)
        .await?;

    tracing::info!(%agent_id, name = ?req.computer_name, "agent enrolled");
    Ok(Json(json!({ "agent_id": agent_id, "token": token })))
}

// ---------- heartbeat ----------

#[derive(Debug, Deserialize)]
pub struct HeartbeatReq {
    #[allow(dead_code)] pub agent_id: Uuid,
}

pub async fn heartbeat(
    State(state): State<AppState>,
    auth: AgentAuth,
    Json(_req): Json<HeartbeatReq>,
) -> Result<Json<Value>, AppError> {
    let agent_id = auth.agent_id;

    sqlx::query("UPDATE agents SET last_seen = now() WHERE agent_id = $1")
        .bind(agent_id)
        .execute(&state.pool)
        .await?;

    // check force_refresh
    let refresh: Option<(bool,)> = sqlx::query_as(
        "UPDATE systems SET force_refresh = false, last_seen = now()
         WHERE agent_id = $1 AND force_refresh = true
         RETURNING true"
    )
    .bind(agent_id)
    .fetch_optional(&state.pool)
    .await?;

    // Update last_seen even if no refresh — but only if systems row exists
    let last_seen: Option<(chrono::DateTime<chrono::Utc>,)> =
        if refresh.is_none() {
            sqlx::query_as(
                "UPDATE systems SET last_seen = now() WHERE agent_id = $1 RETURNING last_seen"
            )
            .bind(agent_id)
            .fetch_optional(&state.pool)
            .await?
        } else {
            // refresh=true path already touched last_seen; fetch it
            sqlx::query_as("SELECT last_seen FROM systems WHERE agent_id = $1")
                .bind(agent_id)
                .fetch_optional(&state.pool)
                .await?
        };

    // Broadcast lightweight heartbeat so dashboards keep the row fresh
    if let Some((ts,)) = last_seen {
        let _ = state.tx.send(SystemEvent::Heartbeat { agent_id, last_seen: ts });
    }

    Ok(Json(json!({ "full_report": refresh.is_some() })))
}

// ---------- ingest ----------

#[derive(Debug, Deserialize)]
pub struct IngestReq {
    #[allow(dead_code)] pub agent_id: Uuid,
    pub collected_at: DateTime<Utc>,
    pub system: SystemPayload,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SystemPayload {
    pub computer_name: String,
    #[serde(default)] pub fqdn: String,
    #[serde(default)] pub domain: String,
    #[serde(default)] pub ip_addresses: Vec<String>,
    #[serde(default)] pub mac_addresses: Vec<String>,
    #[serde(default)] pub platform: String,
    pub console_user: Option<String>,
    #[serde(default)] pub os: OsPayload,
    #[serde(default)] pub cpu_model: String,
    #[serde(default)] pub cpu_cores: u32,
    #[serde(default)] pub ram_bytes: u64,
    #[serde(default)] pub system_type: String,
    #[serde(default)] pub motherboard: String,
    #[serde(default)] pub physical_disk_bytes: u64,
    #[serde(default)] pub drives: Vec<DrivePayload>,
    #[serde(default)] pub installed_software: Vec<Value>,
    #[serde(default)] pub updates: Vec<Value>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct OsPayload {
    #[serde(default)] pub name: String,
    #[serde(default)] pub version: String,
    #[serde(default)] pub build: String,
    #[serde(default)] pub arch: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DrivePayload {
    pub letter: String,
    #[serde(default)] pub label: String,
    #[serde(default)] pub fs: String,
    pub free: u64,
    pub total: u64,
    #[serde(default)] pub is_system: bool,
}

pub async fn ingest(
    State(state): State<AppState>,
    auth: AgentAuth,
    Json(req): Json<IngestReq>,
) -> Result<axum::http::StatusCode, AppError> {
    let agent_id = auth.agent_id;
    let sys = req.system;

    let disk_count = sys.drives.len() as i32;
    let system_drive = sys.drives.iter().find(|d| d.is_system);
    let system_drive_free = system_drive.map(|d| d.free as i64).unwrap_or(0);
    let system_drive_total = system_drive.map(|d| d.total as i64).unwrap_or(0);
    let software_count = sys.installed_software.len() as i32;
    let update_count = sys.updates.len() as i32;

    let drives_json = SqlJson(serde_json::to_value(&sys.drives).unwrap_or(Value::Null));
    let software_json = SqlJson(Value::Array(sys.installed_software.clone()));
    let updates_json = SqlJson(Value::Array(sys.updates.clone()));
    let raw_json = SqlJson(serde_json::to_value(&sys).unwrap_or(Value::Null));

    let ips: Vec<String> = sys
        .ip_addresses
        .iter()
        .filter(|s| s.parse::<std::net::IpAddr>().is_ok())
        .cloned()
        .collect();
    let macs: Vec<String> = sys
        .mac_addresses
        .iter()
        .filter(|m| m.split(':').count() == 6)
        .cloned()
        .collect();

    sqlx::query(
        r#"INSERT INTO systems (
              agent_id, computer_name, fqdn, domain, ip_addresses, mac_addresses,
              platform, console_user,
              os_name, os_version, os_build, os_arch,
              cpu_model, cpu_cores, ram_bytes, system_type, motherboard,
              drives, disk_count, system_drive_free, system_drive_total, main_storage_size,
              installed_software, software_count, updates, update_count,
              last_seen, reported_at, raw_json
          ) VALUES (
              $1,$2,$3,$4,$5::inet[],$6::macaddr[],
              $7,$8,
              $9,$10,$11,$12,
              $13,$14,$15,$16,$17,
              $18,$19,$20,$21,$22,
              $23,$24,$25,$26,
              now(), $27, $28
          )
          ON CONFLICT (agent_id) DO UPDATE SET
              computer_name = EXCLUDED.computer_name,
              fqdn = EXCLUDED.fqdn,
              domain = EXCLUDED.domain,
              ip_addresses = EXCLUDED.ip_addresses,
              mac_addresses = EXCLUDED.mac_addresses,
              platform = EXCLUDED.platform,
              console_user = EXCLUDED.console_user,
              os_name = EXCLUDED.os_name,
              os_version = EXCLUDED.os_version,
              os_build = EXCLUDED.os_build,
              os_arch = EXCLUDED.os_arch,
              cpu_model = EXCLUDED.cpu_model,
              cpu_cores = EXCLUDED.cpu_cores,
              ram_bytes = EXCLUDED.ram_bytes,
              system_type = EXCLUDED.system_type,
              motherboard = EXCLUDED.motherboard,
              drives = EXCLUDED.drives,
              disk_count = EXCLUDED.disk_count,
              system_drive_free = EXCLUDED.system_drive_free,
              system_drive_total = EXCLUDED.system_drive_total,
              main_storage_size = EXCLUDED.main_storage_size,
              installed_software = EXCLUDED.installed_software,
              software_count = EXCLUDED.software_count,
              updates = EXCLUDED.updates,
              update_count = EXCLUDED.update_count,
              last_seen = now(),
              reported_at = EXCLUDED.reported_at,
              raw_json = EXCLUDED.raw_json
          "#
    )
    .bind(agent_id)
    .bind(&sys.computer_name)
    .bind(&sys.fqdn)
    .bind(&sys.domain)
    .bind(&ips)
    .bind(&macs)
    .bind(&sys.platform)
    .bind(&sys.console_user)
    .bind(&sys.os.name)
    .bind(&sys.os.version)
    .bind(&sys.os.build)
    .bind(&sys.os.arch)
    .bind(&sys.cpu_model)
    .bind(sys.cpu_cores as i32)
    .bind(sys.ram_bytes as i64)
    .bind(&sys.system_type)
    .bind(&sys.motherboard)
    .bind(drives_json)
    .bind(disk_count)
    .bind(system_drive_free)
    .bind(system_drive_total)
    .bind(sys.physical_disk_bytes as i64)
    .bind(software_json)
    .bind(software_count)
    .bind(updates_json)
    .bind(update_count)
    .bind(req.collected_at)
    .bind(raw_json)
    .execute(&state.pool)
    .await?;

    let _ = sqlx::query("UPDATE agents SET computer_name = $1 WHERE agent_id = $2")
        .bind(&sys.computer_name)
        .bind(agent_id)
        .execute(&state.pool)
        .await;

    let row: Option<SqlJson<Value>> = sqlx::query_scalar(
        "SELECT to_jsonb(s) FROM systems_current s WHERE agent_id = $1"
    )
    .bind(agent_id)
    .fetch_optional(&state.pool)
    .await?;

    if let Some(SqlJson(v)) = row {
        let _ = state.tx.send(SystemEvent::Ingest {
            agent_id,
            system: v,
        });
    }

    tracing::info!(%agent_id, host = %sys.computer_name, ips = ?sys.ip_addresses, "ingested");
    Ok(axum::http::StatusCode::NO_CONTENT)
}
