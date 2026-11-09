use anyhow::{Context, Result};
use serde::Deserialize;
use wmi::{COMLibrary, WMIConnection};
use winreg::enums::*;
use winreg::RegKey;

use crate::types::{SystemInfo, Update};

/// Owns the WMI connection. `COMLibrary` is moved into `WMIConnection`
/// (it keeps the COM apartment alive internally), so we only store the
/// connection here.
pub struct ComGuard {
    conn: WMIConnection,
}

impl ComGuard {
    pub fn new() -> Result<Self> {
        let com = COMLibrary::new().context("COM init")?;
        let conn = WMIConnection::with_namespace_path("ROOT\\CIMV2", com)
            .context("open WMI ROOT\\CIMV2")?;
        Ok(Self { conn })
    }

    pub fn conn(&self) -> &WMIConnection {
        &self.conn
    }
}

// ---------- rows ----------

#[derive(Debug, Deserialize)]
struct CsRow {
    #[serde(rename = "Name")] name: Option<String>,
    #[serde(rename = "Domain")] domain: Option<String>,
    #[serde(rename = "Workgroup")] workgroup: Option<String>,
    #[serde(rename = "UserName")] user: Option<String>,
    #[serde(rename = "Manufacturer")] manufacturer: Option<String>,
    #[serde(rename = "Model")] model: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OsRow {
    #[serde(rename = "Caption")] caption: Option<String>,
    #[serde(rename = "Version")] version: Option<String>,
    #[serde(rename = "BuildNumber")] build: Option<String>,
    #[serde(rename = "OSArchitecture")] arch: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CpuRow {
    #[serde(rename = "Name")] name: Option<String>,
    #[serde(rename = "NumberOfCores")] cores: Option<u32>,
    #[serde(rename = "NumberOfLogicalProcessors")] logical: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct MemRow {
    #[serde(rename = "TotalPhysicalMemory")] total: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct BoardRow {
    #[serde(rename = "Manufacturer")] manufacturer: Option<String>,
    #[serde(rename = "Product")] product: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DiskRow {
    #[serde(rename = "Size")] size: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct NetRow {
    #[serde(rename = "MACAddress")] mac: Option<String>,
    #[serde(rename = "IPAddress")] ips: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct QfeRow {
    #[serde(rename = "HotFixID")] kb: Option<String>,
    #[serde(rename = "Description")] desc: Option<String>,
    #[serde(rename = "InstalledOn")] on: Option<String>,
}

// ---------- fillers ----------

pub fn fill_system(wmi: &WMIConnection, sys: &mut SystemInfo) -> Result<()> {
    let cs: Vec<CsRow> = wmi.raw_query(
        "SELECT Name, Domain, Workgroup, UserName, Manufacturer, Model FROM Win32_ComputerSystem"
    )?;
    if let Some(cs) = cs.into_iter().next() {
        let name = cs.name.clone().unwrap_or_default();
        let domain = cs.domain.clone()
            .filter(|d| !d.eq_ignore_ascii_case("WORKGROUP"))
            .unwrap_or_default();

        sys.computer_name = name.clone();
        sys.domain = if domain.is_empty() {
            cs.workgroup.clone().unwrap_or_default()
        } else {
            domain.clone()
        };
        sys.fqdn = if domain.is_empty() {
            name
        } else {
            format!("{}.{}", name, domain)
        };
        sys.platform = "Windows".to_string();
        sys.console_user = cs.user.filter(|u| !u.is_empty());
        sys.system_type =
            detect_system_type(cs.manufacturer.as_deref(), cs.model.as_deref());
    }

    let os: Vec<OsRow> = wmi.raw_query(
        "SELECT Caption, Version, BuildNumber, OSArchitecture FROM Win32_OperatingSystem"
    )?;
    if let Some(os) = os.into_iter().next() {
        sys.os.name    = os.caption.unwrap_or_default().replace("Microsoft ", "");
        sys.os.version = os.version.unwrap_or_default();
        sys.os.build   = os.build.unwrap_or_default();
        sys.os.arch    = os.arch.unwrap_or_default();
    }

    let cpu: Vec<CpuRow> = wmi.raw_query(
        "SELECT Name, NumberOfCores, NumberOfLogicalProcessors FROM Win32_Processor"
    )?;
    if let Some(cpu) = cpu.into_iter().next() {
        sys.cpu_model = cpu.name.unwrap_or_default().trim().to_string();
        sys.cpu_cores = cpu.cores.or(cpu.logical).unwrap_or(0);
    }

    let mem: Vec<MemRow> =
        wmi.raw_query("SELECT TotalPhysicalMemory FROM Win32_ComputerSystem")?;
    if let Some(m) = mem.into_iter().next() {
        sys.ram_bytes = m.total.unwrap_or(0);
    }

    let board: Vec<BoardRow> =
        wmi.raw_query("SELECT Manufacturer, Product FROM Win32_BaseBoard")?;
    if let Some(b) = board.into_iter().next() {
        let mfr = b.manufacturer.unwrap_or_default();
        let prod = b.product.unwrap_or_default();
        sys.motherboard = format!("{} ({})", mfr.trim(), prod.trim());
    }

    let disks: Vec<DiskRow> = wmi.raw_query("SELECT Size FROM Win32_DiskDrive")?;
    sys.physical_disk_bytes = disks.iter().filter_map(|d| d.size).sum();

     // --- NEW HEALTH CHECKS ---

    // Uptime (using Win32 GetTickCount64)
    unsafe {
        let tick_count = windows::Win32::System::SystemInformation::GetTickCount64();
        sys.uptime_secs = Some((tick_count / 1000) as i64);
    }

    // Pending Reboot (Registry check)
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let mut pending = false;
    if hklm.open_subkey(r"SOFTWARE\Microsoft\Windows\CurrentVersion\Component Based Servicing\RebootPending").is_ok() {
        pending = true;
    }
    if hklm.open_subkey(r"SOFTWARE\Microsoft\Windows\CurrentVersion\WindowsUpdate\Auto Update\RebootRequired").is_ok() {
        pending = true;
    }
    sys.pending_reboot = Some(pending);

    // GPU Model (Note: raw_query::<GpuRow> already returns Vec<GpuRow>)
    #[derive(Debug, Deserialize)]
    struct GpuRow { #[serde(rename = "Name")] name: Option<String> }
    if let Ok(gpus) = wmi.raw_query::<GpuRow>("SELECT Name FROM Win32_VideoController") {
        let names: Vec<String> = gpus.into_iter().filter_map(|g| g.name).collect();
        if !names.is_empty() {
            sys.gpu_model = Some(names.join(", "));
        }
    }

    // Windows Activation
    #[derive(Debug, Deserialize)]
    struct LicRow { #[serde(rename = "LicenseStatus")] status: Option<u32> }
    if let Ok(lics) = wmi.raw_query::<LicRow>(
        "SELECT LicenseStatus FROM SoftwareLicensingProduct WHERE ApplicationID = '55c92734-d682-4d71-983e-d6ec3f16059f' AND PartialProductKey IS NOT NULL"
    ) {
        if let Some(lic) = lics.first() {
            if let Some(code) = lic.status {
                sys.windows_activation = Some(match code {
                    0 => "Unlicensed".into(),
                    1 => "Activated".into(),
                    2 => "Out-of-Box Grace".into(),
                    3 => "Out-of-Tolerance Grace".into(),
                    4 => "Non-Genuine Grace".into(),
                    5 => "Notification".into(),
                    6 => "Extended Grace".into(),
                    _ => format!("Unknown ({})", code),
                });
            }
        }
    }

    Ok(())
}

pub fn fill_network(wmi: &WMIConnection, sys: &mut SystemInfo) -> Result<()> {
    let rows: Vec<NetRow> = wmi.raw_query(
        "SELECT MACAddress, IPAddress FROM Win32_NetworkAdapterConfiguration WHERE IPEnabled = TRUE"
    )?;

    let mut ips: Vec<String> = Vec::new();
    let mut macs: Vec<String> = Vec::new();

    for r in rows {
        // Normalize MAC: uppercase, hyphens → colons (Postgres macaddr format)
        let mac = r
            .mac
            .unwrap_or_default()
            .trim()
            .to_uppercase()
            .replace('-', ":");
        if mac.is_empty() || mac.split(':').count() != 6 {
            continue;
        }

        let candidate_ips: Vec<String> = r
            .ips
            .unwrap_or_default()
            .into_iter()
            .filter(|ip| {
                !ip.starts_with("127.")
                    && !ip.starts_with("169.254.")
                    && !ip.contains(':')
                    && ip.parse::<std::net::Ipv4Addr>().is_ok()
            })
            .collect();
        if candidate_ips.is_empty() {
            continue;
        }
        if !macs.contains(&mac) {
            macs.push(mac);
        }
        for ip in candidate_ips {
            if !ips.contains(&ip) {
                ips.push(ip);
            }
        }
    }

    sys.ip_addresses = ips;
    sys.mac_addresses = macs;
    Ok(())
}

pub fn fill_updates(wmi: &WMIConnection, sys: &mut SystemInfo) -> Result<()> {
    let rows: Vec<QfeRow> = wmi.raw_query(
        "SELECT HotFixID, Description, InstalledOn FROM Win32_QuickFixEngineering"
    )?;
    sys.updates = rows
        .into_iter()
        .map(|r| Update {
            kb: r.kb.unwrap_or_default(),
            description: r.desc.unwrap_or_default(),
            installed_on: r.on.unwrap_or_default(),
        })
        .collect();
    sys.updates
        .sort_by(|a, b| b.installed_on.cmp(&a.installed_on));
    Ok(())
}

fn detect_system_type(manufacturer: Option<&str>, model: Option<&str>) -> String {
    let mfr = manufacturer.unwrap_or("").to_lowercase();
    let mdl = model.unwrap_or("").to_lowercase();

    const VM_MODEL_HINTS: &[&str] = &[
        "virtualbox", "vmware", "virtual machine", "hyper-v", "kvm",
        "qemu", "xen", "bochs", "parallels",
    ];
    const VM_MFR_HINTS: &[&str] = &[
        "vmware, inc.", "innotek gmbh", "qemu", "xen",
        "parallels software international", "microsoft corporation",
    ];

    if VM_MODEL_HINTS.iter().any(|v| mdl.contains(v))
        || VM_MFR_HINTS.iter().any(|v| mfr.contains(v))
    {
        "VM".into()
    } else {
        "Desktop".into()
    }
}
