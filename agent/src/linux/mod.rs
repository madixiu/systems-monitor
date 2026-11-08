pub mod disks;
pub mod network;
pub mod software;
pub mod system;

use anyhow::Result;
use chrono::Utc;
use uuid::Uuid;

use crate::types::{Report, SystemInfo};

pub fn collect_sync(agent_id: Uuid) -> Result<Report> {
    let mut sys = SystemInfo::default();

    macro_rules! try_collect {
        ($label:expr, $block:expr) => {
            if let Err(e) = $block {
                tracing::warn!("collect {}: {:#}", $label, e);
            }
        };
    }

    try_collect!("system",  system::fill(&mut sys));
    try_collect!("network", network::fill(&mut sys));
    try_collect!("disks",   disks::fill(&mut sys));
    try_collect!("software", software::fill(&mut sys));

    Ok(Report {
        agent_id,
        collected_at: Utc::now(),
        refresh_generation: None,
        system: sys,
    })
}