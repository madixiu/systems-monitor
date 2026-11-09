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
            last_report_at: None,
        })
    }

    pub async fn heartbeat(&self, state: &State) -> Result<HeartbeatResponse> {
        let (user, uptime) = tokio::task::spawn_blocking(|| {
            (crate::collect::current_console_user(), crate::collect::uptime_secs())
        }).await.context("collect heartbeat observations")?;
        let session = match user {
            Ok(user) => Some(json!({ "user": user })),
            Err(error) => {
                tracing::warn!("collect heartbeat user: {error:#}");
                None
            }
        };
        let uptime_secs = match uptime {
            Ok(uptime) => Some(uptime),
            Err(error) => {
                tracing::warn!("collect heartbeat uptime: {error:#}");
                None
            }
        };
        let resp = self.http
            .post(format!("{}/api/v1/agent/heartbeat", self.base))
            .bearer_auth(&state.token)
            .json(&json!({ "agent_id": state.agent_id, "session": session, "uptime_secs": uptime_secs }))
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
