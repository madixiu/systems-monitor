use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::json;
use std::time::Duration;

use crate::config::Config;
use crate::state::State;
use crate::types::{EnrollResponse, HeartbeatResponse, Report};

pub struct ReportClient {
    http: Client,
    base: String,
}

impl ReportClient {
    pub fn new(cfg: &Config) -> Result<Self> {
        let http = Client::builder()
            .timeout(Duration::from_secs(20))
            .build()?;
        Ok(Self { http, base: cfg.server.url.trim_end_matches('/').to_string() })
    }

    pub async fn enroll(&self, secret: &str) -> Result<State> {
        let hostname = std::env::var("COMPUTERNAME").unwrap_or_default();
        let resp = self.http
            .post(format!("{}/api/v1/agent/enroll", self.base))
            .bearer_auth(secret)
            .json(&json!({ "computer_name": hostname }))
            .send().await
            .context("enroll send")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("enroll failed {}: {}", status, body);
        }

        let body: EnrollResponse = resp.json().await.context("enroll parse")?;
        Ok(State {
            agent_id: body.agent_id,
            token: body.token,
            enrolled_at: chrono::Utc::now(),
        })
    }

    pub async fn heartbeat(&self, state: &State) -> Result<HeartbeatResponse> {
        let resp = self.http
            .post(format!("{}/api/v1/agent/heartbeat", self.base))
            .bearer_auth(&state.token)
            .json(&json!({ "agent_id": state.agent_id }))
            .send().await
            .context("heartbeat send")?;

        if !resp.status().is_success() {
            let status = resp.status();
            anyhow::bail!("heartbeat {}", status);
        }
        Ok(resp.json().await.context("heartbeat parse")?)
    }

    pub async fn ingest(&self, state: &State, report: &Report) -> Result<()> {
        let resp = self.http
            .post(format!("{}/api/v1/agent/ingest", self.base))
            .bearer_auth(&state.token)
            .json(report)
            .send().await
            .context("ingest send")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("ingest {}: {}", status, body);
        }
        Ok(())
    }
}
