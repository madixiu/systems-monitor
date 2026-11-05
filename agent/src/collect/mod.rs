pub mod disks;
pub mod software;
pub mod wmi;

use anyhow::Result;
use chrono::Utc;
use uuid::Uuid;

use crate::types::{Report, SystemInfo};

pub fn collect_sync(agent_id: Uuid) -> Result<Report> {
    let mut sys = SystemInfo::default();

    // Each collector is best-effort: log + continue on failure.
    macro_rules! try_collect {
        ($label:expr, $block:expr) => {
            if let Err(e) = $block {
                tracing::warn!("collect {}: {:#}", $label, e);
            }
        };
    }

    let com = match wmi::ComGuard::new() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("init WMI: {:#}", e);
            return Ok(Report {
                agent_id,
                collected_at: Utc::now(),
                system: sys,
            });
        }
    };
    let conn = com.conn();

    try_collect!("system",   wmi::fill_system(conn, &mut sys));
    try_collect!("network",  wmi::fill_network(conn, &mut sys));
    try_collect!("updates",  wmi::fill_updates(conn, &mut sys));
    try_collect!("disks",    disks::fill(&mut sys));
    try_collect!("software", software::fill(&mut sys));

    Ok(Report {
        agent_id,
        collected_at: Utc::now(),
        system: sys,
    })
}
