use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct SystemEvent {
    pub kind: &'static str,
    pub agent_id: Uuid,
    pub system: Value,
}
