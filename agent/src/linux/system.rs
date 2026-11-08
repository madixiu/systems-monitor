use anyhow::Result;
use std::fs;
use sysinfo::System;

use crate::types::SystemInfo;

pub fn fill(sys: &mut SystemInfo) -> Result<()> {
    // hostname
    let hostname = std::process::Command::new("hostname").output()?.stdout;
    sys.computer_name = String::from_utf8_lossy(&hostname).trim().to_string();

    // FQDN
    let fqdn = std::process::Command::new("hostname").arg("-f").output();
    if let Ok(out) = fqdn {
        let f = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !f.is_empty() && !f.contains(' ') {
            sys.fqdn = f.clone();
            if let Some((_, domain)) = f.split_once('.') {
                sys.domain = domain.to_string();
            }
        } else {
            sys.fqdn = sys.computer_name.clone();
        }
    }

    sys.platform = "Linux".to_string();
    sys.console_user = who_user();

    // OS
    if let Ok(release) = fs::read_to_string("/etc/os-release") {
        for line in release.lines() {
            if let Some(v) = line.strip_prefix("PRETTY_NAME=") {
                sys.os.name = v.trim_matches('"').to_string();
            }
            if let Some(v) = line.strip_prefix("VERSION_ID=") {
                sys.os.version = v.trim_matches('"').to_string();
            }
        }
    }
    let uname = nix::sys::utsname::uname().ok();
    if let Some(u) = uname {
        sys.os.build = u.release().to_string_lossy().to_string();
    }
    sys.os.arch = std::env::consts::ARCH.to_string();

    // CPU + RAM
    let mut sysinfo = System::new_all();
    sysinfo.refresh_all();
    if let Some(cpu) = sysinfo.cpus().first() {
        sys.cpu_model = cpu.brand().trim().to_string();
    }
    sys.cpu_cores = sysinfo.cpus().len() as u32;
    sys.ram_bytes = sysinfo.total_memory(); // sysinfo 0.32 returns bytes

    // Motherboard
    let vendor = fs::read_to_string("/sys/class/dmi/id/board_vendor").unwrap_or_default();
    let board  = fs::read_to_string("/sys/class/dmi/id/board_name").unwrap_or_default();
    sys.motherboard = format!("{} ({})", vendor.trim(), board.trim());

    // VM detection
    let product = fs::read_to_string("/sys/class/dmi/id/product_name").unwrap_or_default();
    let sys_vendor = fs::read_to_string("/sys/class/dmi/id/sys_vendor").unwrap_or_default();
    sys.system_type = detect_vm(&product, &sys_vendor);

    // Physical disk size
    if let Ok(out) = std::process::Command::new("lsblk")
        .args(["-b", "-d", "-n", "-o", "SIZE,TYPE"])
        .output()
    {
        let mut total: u64 = 0;
        for line in String::from_utf8_lossy(&out.stdout).lines() {
            let mut parts = line.split_whitespace();
            if let (Some(size), Some(typ)) = (parts.next(), parts.next()) {
                if typ == "disk" {
                    total += size.parse::<u64>().unwrap_or(0);
                }
            }
        }
        sys.physical_disk_bytes = total;
    }

    Ok(())
}

fn who_user() -> Option<String> {
    let out = std::process::Command::new("who").output().ok()?;
    let s = String::from_utf8_lossy(&out.stdout);
    for line in s.lines() {
        let mut parts = line.split_whitespace();
        if let Some(user) = parts.next() {
            if !user.is_empty() {
                return Some(user.to_string());
            }
        }
    }
    None
}

fn detect_vm(product: &str, vendor: &str) -> String {
    let p = product.to_lowercase();
    let v = vendor.to_lowercase();
    const HINTS: &[&str] = &[
        "kvm", "vmware", "virtualbox", "innotek", "qemu", "xen", "bochs",
        "parallels", "virtual machine",
    ];
    if HINTS.iter().any(|h| p.contains(h) || v.contains(h)) {
        "VM".into()
    } else {
        "Desktop".into()
    }
}