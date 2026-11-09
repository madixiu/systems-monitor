use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind")]
pub enum SystemEvent {
    #[serde(rename = "ingest")]
    Ingest {
        agent_id: Uuid,
        system: Value,
    },
    #[serde(rename = "heartbeat")]
    Heartbeat {
        agent_id: Uuid,
        last_seen: chrono::DateTime<chrono::Utc>,
        console_user: Option<String>,
        console_user_logged_in: Option<bool>,
        uptime_secs: Option<i64>,
        last_boot: Option<chrono::DateTime<chrono::Utc>>,
    },
}