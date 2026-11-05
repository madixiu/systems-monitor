use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub bind: String,
    pub database_url: String,
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

fn default_log_level() -> String { "info".into() }

impl Config {
    pub fn load() -> Result<Self> {
        // Look for server.toml in CWD, then next to the binary
        let candidates = [
            PathBuf::from("server.toml"),
            std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|d| d.join("server.toml")))
                .unwrap_or_default(),
        ];
        for p in &candidates {
            if p.exists() {
                let text = std::fs::read_to_string(p)
                    .with_context(|| format!("read {}", p.display()))?;
                return toml::from_str(&text).context("parse server.toml");
            }
        }
        anyhow::bail!("server.toml not found (looked in CWD and next to binary)")
    }
}
