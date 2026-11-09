pub mod console_user;
pub mod disks;
pub mod software;
pub mod wmi;

use anyhow::Result;
use chrono::Utc;
use uuid::Uuid;

use crate::types::{Report, SystemInfo};
pub use console_user::current_console_user;

pub fn uptime_secs() -> Result<i64> {
    Ok((unsafe { windows::Win32::System::SystemInformation::GetTickCount64() } / 1000) as i64)
}

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
            match current_console_user() {
                Ok(user) => sys.console_user = user,
                Err(e) => tracing::warn!("collect console user: {e:#}"),
            }
            return Ok(Report {
                agent_id,
                collected_at: Utc::now(),
                refresh_generation: None,
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

    match current_console_user() {
        Ok(user) => sys.console_user = user,
        Err(e) => {
            sys.console_user = None;
            tracing::warn!("collect console user: {e:#}");
        }
    }

    Ok(Report {
        agent_id,
        collected_at: Utc::now(),
        refresh_generation: None,
        system: sys,
    })
}
