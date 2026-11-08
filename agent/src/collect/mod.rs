pub mod console_user;
pub mod disks;
pub mod software;
pub mod wmi;

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

    let com = match wmi::ComGuard::new() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("init WMI: {:#}", e);
            // Still try WTS below — it does not need COM.
            if let Some(u) = console_user::current_console_user() {
                sys.console_user = Some(u);
            }
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

    // Prefer WTS (works from SYSTEM). WMI's value, if any, stays as fallback.
    if let Some(user) = console_user::current_console_user() {
        sys.console_user = Some(user);
    }

    Ok(Report {
        agent_id,
        collected_at: Utc::now(),
        system: sys,
    })
}
