use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct Report {
    pub agent_id: Uuid,
    pub collected_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_generation: Option<i64>,
    pub system: SystemInfo,
}

#[derive(Debug, Serialize, Default)]
pub struct SystemInfo {
    pub computer_name: String,
    pub fqdn: String,
    pub domain: String,
    pub ip_addresses: Vec<String>,
    pub mac_addresses: Vec<String>,
    pub platform: String,
    pub console_user: Option<String>,
    pub os: OsInfo,
    pub cpu_model: String,
    pub cpu_cores: u32,
    pub ram_bytes: u64,
    pub system_type: String,
    pub motherboard: String,
    pub physical_disk_bytes: u64,
    pub drives: Vec<DriveInfo>,
    pub installed_software: Vec<Software>,
    pub updates: Vec<Update>,
}

#[derive(Debug, Serialize, Default)]
pub struct OsInfo {
    pub name: String,
    pub version: String,
    pub build: String,
    pub arch: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct DriveInfo {
    pub letter: String,
    pub label: String,
    pub fs: String,
    pub free: u64,
    pub total: u64,
    pub is_system: bool,
}

#[derive(Debug, Serialize, Clone)]
pub struct Software {
    pub name: String,
    pub version: String,
    pub publisher: String,
    pub install_date: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct Update {
    pub kb: String,
    pub description: String,
    pub installed_on: String,
}

#[derive(Debug, Deserialize)]
pub struct EnrollResponse {
    pub agent_id: Uuid,
    pub token: String,
}

#[derive(Debug, Deserialize)]
pub struct HeartbeatResponse {
    pub full_report: bool,
    #[serde(default)]
    pub refresh_generation: Option<i64>,
}
