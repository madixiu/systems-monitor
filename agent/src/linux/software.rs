use anyhow::Result;
use std::process::Command;

use crate::types::{Software, SystemInfo};

pub fn fill(sys: &mut SystemInfo) -> Result<()> {
    let mut sw: Vec<Software> = Vec::new();

    // Debian/Ubuntu
    if which("dpkg").is_some() {
        if let Ok(out) = Command::new("dpkg-query")
            .args(["-W", "-f=${Package}\t${Version}\t${Maintainer}\n"])
            .output()
        {
            for line in String::from_utf8_lossy(&out.stdout).lines() {
                let parts: Vec<&str> = line.split('\t').collect();
                if parts.len() >= 2 {
                    sw.push(Software {
                        name: parts[0].to_string(),
                        version: parts[1].to_string(),
                        publisher: parts.get(2).unwrap_or(&"").to_string(),
                        install_date: String::new(),
                    });
                }
            }
        }
        // pending updates
        if let Ok(out) = Command::new("apt").args(["list", "--upgradable"]).output() {
            let mut updates = Vec::new();
            for line in String::from_utf8_lossy(&out.stdout).lines().skip(1) {
                if let Some(pkg) = line.split('/').next() {
                    updates.push(crate::types::Update {
                        kb: pkg.to_string(),
                        description: "available".into(),
                        installed_on: String::new(),
                    });
                }
            }
            sys.updates = updates;
        }
    }

    // RHEL/CentOS/Fedora
    else if which("rpm").is_some() {
        if let Ok(out) = Command::new("rpm")
            .args(["-qa", "--queryformat", "%{NAME}\t%{VERSION}\t%{VENDOR}\n"])
            .output()
        {
            for line in String::from_utf8_lossy(&out.stdout).lines() {
                let parts: Vec<&str> = line.split('\t').collect();
                if parts.len() >= 2 {
                    sw.push(Software {
                        name: parts[0].to_string(),
                        version: parts[1].to_string(),
                        publisher: parts.get(2).unwrap_or(&"(none)").to_string(),
                        install_date: String::new(),
                    });
                }
            }
        }
        if let Ok(out) = Command::new("dnf").args(["check-update"]).output() {
            let mut updates = Vec::new();
            for line in String::from_utf8_lossy(&out.stdout).lines() {
                if let Some(pkg) = line.split_whitespace().next() {
                    if !pkg.is_empty() && !line.starts_with(' ') {
                        updates.push(crate::types::Update {
                            kb: pkg.to_string(),
                            description: "available".into(),
                            installed_on: String::new(),
                        });
                    }
                }
            }
            sys.updates = updates;
        }
    }

    // Arch (Manjaro)
    else if which("pacman").is_some() {
        if let Ok(out) = Command::new("pacman").arg("-Q").output() {
            for line in String::from_utf8_lossy(&out.stdout).lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    sw.push(Software {
                        name: parts[0].to_string(),
                        version: parts[1].to_string(),
                        publisher: String::new(),
                        install_date: String::new(),
                    });
                }
            }
        }
        if let Ok(out) = Command::new("pacman").arg("-Qu").output() {
            let mut updates = Vec::new();
            for line in String::from_utf8_lossy(&out.stdout).lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    updates.push(crate::types::Update {
                        kb: parts[0].to_string(),
                        description: format!("{} → {}", parts[1], parts.get(3).unwrap_or(&"?")),
                        installed_on: String::new(),
                    });
                }
            }
            sys.updates = updates;
        }
    }

    sw.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    if sw.len() > 3000 { sw.truncate(3000); }

    sys.installed_software = sw;
    Ok(())
}

fn which(cmd: &str) -> Option<std::path::PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths).find_map(|p| {
            let full = p.join(cmd);
            if full.is_file() { Some(full) } else { None }
        })
    })
}