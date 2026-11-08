use anyhow::Result;
use nix::sys::statvfs::statvfs;
use std::fs;

use crate::types::{DriveInfo, SystemInfo};

pub fn fill(sys: &mut SystemInfo) -> Result<()> {
    let mut out: Vec<DriveInfo> = Vec::new();

    let mounts = fs::read_to_string("/proc/mounts")?;
    for line in mounts.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 3 { continue; }

        let device = fields[0];
        let mount = fields[1];
        let fstype = fields[2];

        // Only real block devices (skip proc, sys, tmpfs, overlay, snap, etc.)
        if !device.starts_with("/dev/") { continue; }
        if !["ext4", "ext3", "ext2", "xfs", "btrfs", "f2fs", "zfs"].contains(&fstype) { continue; }

        if let Ok(stat) = statvfs(mount) {
            let total = stat.blocks() as u64 * stat.fragment_size() as u64;
            let free = stat.blocks_available() as u64 * stat.fragment_size() as u64;
            if total == 0 { continue; }

            out.push(DriveInfo {
                letter: mount.to_string(),       // mount path as "letter" (display)
                label: device.to_string(),       // device path as "label"
                fs: fstype.to_string(),
                free,
                total,
                is_system: mount == "/",
            });
        }
    }

    sys.drives = out;
    Ok(())
}