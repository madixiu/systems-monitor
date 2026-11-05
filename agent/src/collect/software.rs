use anyhow::Result;
use winreg::enums::*;
use winreg::RegKey;

use crate::types::{Software, SystemInfo};

const UNINSTALL_PATH: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall";

pub fn fill(sys: &mut SystemInfo) -> Result<()> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let mut out: Vec<Software> = Vec::new();

    let views: [u32; 2] = [KEY_WOW64_64KEY, KEY_WOW64_32KEY];

    for wow in views {
        let uninstall = match hklm.open_subkey_with_flags(UNINSTALL_PATH, KEY_READ | wow) {
            Ok(k) => k,
            Err(_) => continue,
        };

        for sub_name in uninstall.enum_keys().flatten() {
            let sub = match uninstall.open_subkey_with_flags(&sub_name, KEY_READ | wow) {
                Ok(k) => k,
                Err(_) => continue,
            };

            let name: String = sub.get_value("DisplayName").unwrap_or_default();
            if name.trim().is_empty() { continue; }

            // Skip Windows internal components
            let sys_comp: u32 = sub.get_value("SystemComponent").unwrap_or(0);
            if sys_comp == 1 { continue; }
            let parent_kb: String = sub.get_value("ParentKeyName").unwrap_or_default();
            if !parent_kb.is_empty() { continue; }
            // Skip security updates that masquerade as apps
            if name.to_lowercase().starts_with("security update for") { continue; }
            if name.to_lowercase().starts_with("update for") { continue; }

            let version:   String = sub.get_value("DisplayVersion").unwrap_or_default();
            let publisher: String = sub.get_value("Publisher").unwrap_or_default();
            let install:   String = sub.get_value("InstallDate").unwrap_or_default();

            out.push(Software {
                name: name.trim().to_string(),
                version,
                publisher,
                install_date: install,
            });
        }
    }

    // De-dupe: same (name, version) seen in both views
    out.sort_by(|a, b| {
        a.name.to_lowercase().cmp(&b.name.to_lowercase())
            .then(a.version.cmp(&b.version))
    });
    out.dedup_by(|a, b| {
        a.name.eq_ignore_ascii_case(&b.name) && a.version == b.version
    });

    // Truncate pathological lists (flagged on server via software_count)
    if out.len() > 1000 { out.truncate(1000); }

    sys.installed_software = out;
    Ok(())
}
