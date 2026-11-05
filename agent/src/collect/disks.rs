use anyhow::Result;
use windows::core::PCWSTR;
use windows::Win32::Storage::FileSystem::{
    GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDrives, GetVolumeInformationW,
};

use crate::types::{DriveInfo, SystemInfo};

// Stable Win32 ABI values for GetDriveTypeW() return.
const DRIVE_FIXED: u32 = 3;

pub fn fill(sys: &mut SystemInfo) -> Result<()> {
    let mask = unsafe { GetLogicalDrives() };
    let system_drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
    let mut out = Vec::new();

    for i in 0..26u8 {
        if mask & (1 << i) == 0 {
            continue;
        }
        let letter = format!("{}:", (b'A' + i) as char);
        let root = format!("{}\\", letter);
        let wide: Vec<u16> = root.encode_utf16().chain(std::iter::once(0)).collect();
        let pcw = PCWSTR(wide.as_ptr());

        let dtype = unsafe { GetDriveTypeW(pcw) };
        if dtype != DRIVE_FIXED {
            continue;
        }

        let mut free_to_caller: u64 = 0;
        let mut total: u64 = 0;
        let mut total_free: u64 = 0;
        let ok = unsafe {
            GetDiskFreeSpaceExW(
                pcw,
                Some(&mut free_to_caller),
                Some(&mut total),
                Some(&mut total_free),
            )
        };
        if ok.is_err() || total == 0 {
            continue;
        }

        let mut label_buf = [0u16; 256];
        let mut fs_buf = [0u16; 64];
        let _ = unsafe {
            GetVolumeInformationW(
                pcw,
                Some(&mut label_buf),
                None,
                None,
                None,
                Some(&mut fs_buf),
            )
        };
        let label = String::from_utf16_lossy(&label_buf)
            .trim_end_matches('\0')
            .to_string();
        let fs = String::from_utf16_lossy(&fs_buf)
            .trim_end_matches('\0')
            .to_string();

        out.push(DriveInfo {
            letter: letter.clone(),
            label,
            fs,
            free: free_to_caller,
            total,
            is_system: letter.eq_ignore_ascii_case(&system_drive),
        });
    }

    sys.drives = out;
    Ok(())
}
