use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;
use uuid::Uuid;

use crate::config::{state_path, Config};
use crate::report::ReportClient;

#[derive(Debug, Serialize, Deserialize)]
pub struct State {
    pub agent_id: Uuid,
    pub token: String,
    pub enrolled_at: DateTime<Utc>,
}

impl State {
    pub fn load() -> Result<Option<Self>> {
        let path = state_path();
        if !path.exists() { return Ok(None); }
        let text = std::fs::read_to_string(&path)?;
        Ok(Some(serde_json::from_str(&text).context("parse state.json")?))
    }

    pub fn save(&self) -> Result<()> {
        let path = state_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, text)?;
        // tighten ACL later during deploy step
        Ok(())
    }

    pub async fn load_or_enroll(cfg: &Config, client: &ReportClient) -> Result<Self> {
        if let Some(s) = Self::load()? {
            return Ok(s);
        }
        let s = client.enroll(&cfg.server.enrollment_secret).await?;
        s.save()?;
        Ok(s)
    }
}

#[allow(dead_code)]
fn _unused(_: &Path) {}
