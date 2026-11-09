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
    #[serde(default)] pub session: Option<SessionObservation>,
    #[serde(default)] pub uptime_secs: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct SessionObservation {
    pub user: Option<String>,
}

pub async fn heartbeat(
    State(state): State<AppState>,
    auth: AgentAuth,
    Json(req): Json<HeartbeatReq>,
) -> Result<Json<Value>, AppError> {
    let agent_id = auth.agent_id;

    let user = req.session.as_ref().and_then(|session| session.user.as_deref());
    if user.is_some_and(|user| user.trim().is_empty()) {
        return Err(AppError::BadRequest("session user must not be empty".into()));
    }
    let logged_in = req.session.as_ref().map(|session| session.user.is_some());
    let now = Utc::now();
    let last_boot = match req.uptime_secs {
        Some(seconds) if seconds >= 0 => Some(
            chrono::Duration::try_seconds(seconds)
                .and_then(|duration| now.checked_sub_signed(duration))
                .ok_or_else(|| AppError::BadRequest("invalid uptime".into()))?,
        ),
        Some(_) => return Err(AppError::BadRequest("invalid uptime".into())),
        None => None,
    };

    sqlx::query("UPDATE agents SET last_seen = now() WHERE agent_id = $1")
        .bind(agent_id)
        .execute(&state.pool)
        .await?;

    let report_interval: Option<(Value,)> = sqlx::query_as(
        "SELECT value FROM settings WHERE key = 'report_interval_secs'"
    )
    .fetch_optional(&state.pool)
    .await?;
    let report_interval_secs = report_interval
        .and_then(|(value,)| {
            value.as_i64().or_else(|| value.as_str()?.parse::<i64>().ok())
        })
        .unwrap_or(86_400)
        .max(60);

    let system: Option<(bool, i64, Option<DateTime<Utc>>, DateTime<Utc>, Option<String>, Option<bool>, Option<i64>, Option<DateTime<Utc>>)> = sqlx::query_as(
        "UPDATE systems SET last_seen = now(),
             console_user = COALESCE($2, console_user),
             console_user_logged_in = $3,
             uptime_secs = COALESCE($4, uptime_secs),
             last_boot = COALESCE($5, last_boot)
         WHERE agent_id = $1
         RETURNING force_refresh, force_refresh_generation, reported_at, last_seen,
                   console_user, console_user_logged_in, uptime_secs, last_boot"
    )
    .bind(agent_id)
    .bind(user)
    .bind(logged_in)
    .bind(req.uptime_secs)
    .bind(last_boot)
    .fetch_optional(&state.pool)
    .await?;

    // Broadcast lightweight heartbeat so dashboards keep the row fresh
    if let Some((_, _, _, ts, user, logged_in, uptime, boot)) = system.as_ref() {
        let _ = state.tx.send(SystemEvent::Heartbeat {
            agent_id,
            last_seen: *ts,
            console_user: user.clone(),
            console_user_logged_in: *logged_in,
            uptime_secs: *uptime,
            last_boot: *boot,
        });
    }

    let full_report = system
        .as_ref()
        .map(|(refresh, _, reported_at, _, _, _, _, _)| {
            *refresh
                || reported_at
                    .map(|reported| {
                        Utc::now()
                            .signed_duration_since(reported)
                            .num_seconds()
                            >= report_interval_secs
                    })
                    .unwrap_or(true)
        })
        .unwrap_or(true);
    let refresh_generation = system
        .as_ref()
        .and_then(|(refresh, generation, _, _, _, _, _, _)| refresh.then_some(*generation));

    Ok(Json(json!({
        "full_report": full_report,
        "refresh_generation": refresh_generation
    })))
}

// ---------- ingest ----------

#[derive(Debug, Deserialize)]
pub struct IngestReq {
    #[allow(dead_code)] pub agent_id: Uuid,
    pub collected_at: DateTime<Utc>,
    #[serde(default)]
    pub refresh_generation: Option<i64>,
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
    // NEW HEALTH FIELDS
    #[serde(default)] pub uptime_secs: Option<i64>,
    #[serde(default)] pub pending_reboot: Option<bool>,
    #[serde(default)] pub gpu_model: Option<String>,
    #[serde(default)] pub windows_activation: Option<String>,
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
    // Calculate clock skew (server time minus agent time)
    let clock_skew_secs = Utc::now().signed_duration_since(req.collected_at).num_seconds();
    let last_boot = sys.uptime_secs.map(|up| Utc::now() - chrono::Duration::seconds(up));
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
              last_seen, reported_at, raw_json,
              uptime_secs, last_boot, pending_reboot, clock_skew_secs, gpu_model, windows_activation
          ) VALUES (
              $1,$2,$3,$4,$5::inet[],$6::macaddr[],
              $7,NULLIF(BTRIM($8), ''),
              $9,$10,$11,$12,
              $13,$14,$15,$16,$17,
              $18,$19,$20,$21,$22,
              $23,$24,$25,$26,
              now(), $27, $28,
              $29, $30, $31, $32, $33, $34
          )
          ON CONFLICT (agent_id) DO UPDATE SET
              computer_name = EXCLUDED.computer_name,
              fqdn = EXCLUDED.fqdn,
              domain = EXCLUDED.domain,
              ip_addresses = EXCLUDED.ip_addresses,
              mac_addresses = EXCLUDED.mac_addresses,
              platform = EXCLUDED.platform,
              console_user = CASE
                  WHEN systems.console_user_logged_in IS NULL
                  THEN COALESCE(EXCLUDED.console_user, systems.console_user)
                  ELSE systems.console_user
              END,
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
              raw_json = EXCLUDED.raw_json,
              uptime_secs = COALESCE(EXCLUDED.uptime_secs, systems.uptime_secs),
              last_boot = COALESCE(EXCLUDED.last_boot, systems.last_boot),
              pending_reboot = EXCLUDED.pending_reboot,
              clock_skew_secs = EXCLUDED.clock_skew_secs,
              gpu_model = EXCLUDED.gpu_model,
              windows_activation = EXCLUDED.windows_activation
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
    .bind(sys.uptime_secs)
    .bind(last_boot)
    .bind(sys.pending_reboot)
    .bind(clock_skew_secs)
    .bind(&sys.gpu_model)
    .bind(&sys.windows_activation)
    .execute(&state.pool)
    .await?;

    if let Some(generation) = req.refresh_generation {
        sqlx::query(
            "UPDATE systems
             SET force_refresh = false
             WHERE agent_id = $1
               AND force_refresh = true
               AND force_refresh_generation = $2"
        )
        .bind(agent_id)
        .bind(generation)
        .execute(&state.pool)
        .await?;
    }

    let _ = sqlx::query("UPDATE agents SET computer_name = $1 WHERE agent_id = $2")
        .bind(&sys.computer_name)
        .bind(agent_id)
        .execute(&state.pool)
        .await;

    let row: Option<SqlJson<Value>> = sqlx::query_scalar(
        &format!("{} WHERE s.agent_id = $1", crate::db::SYSTEMS_SNAPSHOT_SELECT)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::ApiAuth;
    use crate::routes::{dashboard, export};
    use axum::extract::{Path, Query};
    use sqlx::PgPool;
    use tokio::sync::broadcast;

    #[test]
    fn heartbeat_session_observation_is_backward_compatible() {
        let id = Uuid::new_v4();
        let legacy: HeartbeatReq = serde_json::from_value(json!({ "agent_id": id })).unwrap();
        assert!(legacy.session.is_none());
        let failed: HeartbeatReq = serde_json::from_value(json!({ "agent_id": id, "session": null })).unwrap();
        assert!(failed.session.is_none());
        let signed_out: HeartbeatReq = serde_json::from_value(json!({
            "agent_id": id, "session": { "user": null }, "uptime_secs": 123
        })).unwrap();
        assert!(signed_out.session.unwrap().user.is_none());
        assert_eq!(signed_out.uptime_secs, Some(123));
    }

    #[sqlx::test]
    #[ignore = "requires a PostgreSQL test database"]
    async fn health_fields_reach_dashboard_stream_and_full_export(pool: PgPool) {
        let agent_id = Uuid::new_v4();
        sqlx::query("INSERT INTO agents (agent_id, token_hash) VALUES ($1, $2)")
            .bind(agent_id)
            .bind("test-token-hash")
            .execute(&pool)
            .await
            .unwrap();
        let (tx, mut rx) = broadcast::channel(10);
        let state = AppState { pool, tx };
        let system: SystemPayload = serde_json::from_value(json!({
            "computer_name": "test-host",
            "platform": "Windows",
            "uptime_secs": 0,
            "pending_reboot": false,
            "gpu_model": "Test GPU",
            "windows_activation": "Activated"
        })).unwrap();
        let req = IngestReq {
            agent_id,
            collected_at: Utc::now(),
            refresh_generation: None,
            system,
        };
        ingest(State(state.clone()), AgentAuth { agent_id }, Json(req)).await.unwrap();

        fn check_fields(value: &Value) {
            assert_eq!(value["uptime_secs"], 0);
            assert_eq!(value["pending_reboot"], false);
            assert_eq!(value["gpu_model"], "Test GPU");
            assert_eq!(value["windows_activation"], "Activated");
            assert!(value["last_boot"].is_string());
            assert!(value["clock_skew_secs"].is_number());
            assert_eq!(value["offline_threshold_secs"].as_f64(), Some(600.0));
            assert_eq!(value["live_state"], "online");
        }

        let Json(detail) = dashboard::get_one(State(state.clone()), ApiAuth, Path(agent_id))
            .await.unwrap();
        check_fields(&detail);
        let Json(list) = dashboard::list(State(state.clone()), ApiAuth, Query(Default::default()))
            .await.unwrap();
        check_fields(&list["items"][0]);
        match rx.recv().await.unwrap() {
            SystemEvent::Ingest { system, .. } => check_fields(&system),
            _ => panic!("expected an ingest event"),
        }
        let response = export::export(
            State(state),
            ApiAuth,
            Query(export::ExportParams { full: Some("1".into()), ..Default::default() }),
        ).await.unwrap();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let exported: Value = serde_json::from_slice(&body).unwrap();
        check_fields(&exported[0]);
    }

    #[sqlx::test]
    #[ignore = "requires a PostgreSQL test database"]
    async fn heartbeat_preserves_last_user_and_reports_current_session(pool: PgPool) {
        let agent_id = Uuid::new_v4();
        sqlx::query("INSERT INTO agents (agent_id, token_hash) VALUES ($1, $2)")
            .bind(agent_id).bind("session-test-token").execute(&pool).await.unwrap();
        let (tx, mut rx) = broadcast::channel(16);
        let state = AppState { pool, tx };

        async fn report(state: &AppState, id: Uuid, user: Option<&str>) {
            let system = serde_json::from_value(json!({
                "computer_name": "test-host", "console_user": user
            })).unwrap();
            ingest(State(state.clone()), AgentAuth { agent_id: id }, Json(IngestReq {
                agent_id: id, collected_at: Utc::now(), refresh_generation: None, system,
            })).await.unwrap();
        }
        async fn send(state: &AppState, id: Uuid, session: Option<Option<&str>>, uptime: Option<i64>) {
            let req = HeartbeatReq {
                agent_id: id,
                session: session.map(|user| SessionObservation { user: user.map(str::to_owned) }),
                uptime_secs: uptime,
            };
            let _ = heartbeat(State(state.clone()), AgentAuth { agent_id: id }, Json(req)).await.unwrap();
        }
        async fn snapshot(state: &AppState, id: Uuid) -> Value {
            dashboard::get_one(State(state.clone()), ApiAuth, Path(id)).await.unwrap().0
        }

        report(&state, agent_id, Some("DOMAIN\\alice")).await;
        rx.recv().await.unwrap();
        send(&state, agent_id, Some(Some("DOMAIN\\alice")), Some(90000)).await;
        let s = snapshot(&state, agent_id).await;
        assert_eq!(s["console_user"], "DOMAIN\\alice");
        assert_eq!(s["console_user_logged_in"], true);
        assert_eq!(s["uptime_secs"], 90000);
        match rx.recv().await.unwrap() {
            SystemEvent::Heartbeat { console_user, console_user_logged_in, uptime_secs, last_boot, .. } => {
                assert_eq!(console_user.as_deref(), Some("DOMAIN\\alice"));
                assert_eq!(console_user_logged_in, Some(true));
                assert_eq!(uptime_secs, Some(90000));
                assert!(last_boot.is_some());
            }
            _ => panic!("expected heartbeat event"),
        }

        send(&state, agent_id, Some(None), Some(30)).await;
        let s = snapshot(&state, agent_id).await;
        assert_eq!(s["console_user"], "DOMAIN\\alice");
        assert_eq!(s["console_user_logged_in"], false);
        assert_eq!(s["uptime_secs"], 30);
        report(&state, agent_id, None).await;
        let s = snapshot(&state, agent_id).await;
        assert_eq!(s["console_user"], "DOMAIN\\alice");
        assert_eq!(s["console_user_logged_in"], false);
        assert_eq!(s["uptime_secs"], 30);
        report(&state, agent_id, Some("stale-user")).await;
        assert_eq!(snapshot(&state, agent_id).await["console_user"], "DOMAIN\\alice");

        send(&state, agent_id, Some(Some("DOMAIN\\bob")), None).await;
        let s = snapshot(&state, agent_id).await;
        assert_eq!(s["console_user"], "DOMAIN\\bob");
        assert_eq!(s["console_user_logged_in"], true);
        send(&state, agent_id, None, None).await;
        let s = snapshot(&state, agent_id).await;
        assert_eq!(s["console_user"], "DOMAIN\\bob");
        assert!(s["console_user_logged_in"].is_null());
        report(&state, agent_id, None).await;
        assert_eq!(snapshot(&state, agent_id).await["console_user"], "DOMAIN\\bob");

        let invalid = HeartbeatReq { agent_id, session: None, uptime_secs: Some(-1) };
        assert!(matches!(
            heartbeat(State(state), AgentAuth { agent_id }, Json(invalid)).await,
            Err(AppError::BadRequest(_))
        ));
    }
}
