use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::PathBuf;

#[cfg(target_os = "windows")]
pub const CONFIG_DIR: &str = r"C:\ProgramData\PakMonitor";

#[cfg(target_os = "linux")]
pub const CONFIG_DIR: &str = "/etc/pak-monitor";

pub fn config_path() -> PathBuf {
    PathBuf::from(CONFIG_DIR).join("agent.toml")
}

pub fn state_path() -> PathBuf {
    PathBuf::from(CONFIG_DIR).join("state.json")
}

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub server: ServerConfig,
    #[serde(default)]
    pub interval: IntervalConfig,
    #[serde(default)]
    pub log: LogConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    pub url: String,
    pub enrollment_secret: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct IntervalConfig {
    #[serde(default = "default_heartbeat")]
    pub heartbeat_secs: u64,
    #[serde(default = "default_report")]
    pub report_secs: u64,
}
fn default_heartbeat() -> u64 { 30 }
fn default_report()    -> u64 { 300 }
impl Default for IntervalConfig {
    fn default() -> Self {
        Self { heartbeat_secs: default_heartbeat(), report_secs: default_report() }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct LogConfig {
    #[serde(default = "default_level")]
    pub level: String,
}
fn default_level() -> String { "info".into() }
impl Default for LogConfig {
    fn default() -> Self { Self { level: default_level() } }
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = config_path();
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("read {}", path.display()))?;
        toml::from_str(&text).context("parse agent.toml")
    }
}