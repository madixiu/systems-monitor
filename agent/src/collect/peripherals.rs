use serde::Deserialize;
use winreg::enums::HKEY_LOCAL_MACHINE;
use winreg::RegKey;
use wmi::WMIConnection;

use crate::types::{PeripheralDevice, PeripheralInventory, PrinterInfo, SystemInfo};

const CLASS_MONITOR: &str = "{4d36e96e-e325-11ce-bfc1-08002be10318}";
const CLASS_IMAGING: &str = "{6bdd1fc6-810f-11d0-bec7-08002be2092f}";

// Ports used by Windows' built-in software printers, not by hardware.
const VIRTUAL_PORTS: &[&str] = &["portprompt:", "nul:", "shrfax:", "file:", "xpsport:"];

#[derive(Debug, Deserialize)]
struct PrinterRow {
    #[serde(rename = "Name")]
    name: Option<String>,
    #[serde(rename = "DriverName")]
    driver: Option<String>,
    #[serde(rename = "PortName")]
    port: Option<String>,
    #[serde(rename = "ServerName")]
    server: Option<String>,
    #[serde(rename = "Network")]
    network: Option<bool>,
    #[serde(rename = "Shared")]
    shared: Option<bool>,
    #[serde(rename = "PrinterStatus")]
    status: Option<u16>,
}

#[derive(Debug, Deserialize)]
struct DeviceRow {
    #[serde(rename = "Name")]
    name: Option<String>,
    #[serde(rename = "ClassGuid")]
    class: Option<String>,
    #[serde(rename = "Manufacturer")]
    manufacturer: Option<String>,
    #[serde(rename = "DeviceID")]
    device_id: Option<String>,
    #[serde(rename = "Status")]
    status: Option<String>,
}

fn printer_info(row: PrinterRow) -> Option<PrinterInfo> {
    // PrinterStatus 7 = Offline
    if row.status == Some(7) || is_virtual_port(row.port.as_deref()) {
        return None;
    }
    Some(PrinterInfo {
        name: row.name.unwrap_or_default(),
        driver: row.driver,
        port: row.port,
        server: row.server,
        network: row.network,
        shared: row.shared,
        status: row.status,
    })
}

fn is_virtual_port(port: Option<&str>) -> bool {
    port.is_some_and(|p| VIRTUAL_PORTS.contains(&p.trim().to_ascii_lowercase().as_str()))
}

struct Edid {
    manufacturer: String,
    model: Option<String>,
    serial: Option<String>,
    resolution: Option<String>,
}

fn parse_edid(edid: &[u8]) -> Option<Edid> {
    if edid.len() < 128 || edid[..8] != [0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0] {
        return None;
    }
    let id = u16::from_be_bytes([edid[8], edid[9]]);
    let manufacturer: String = [10, 5, 0]
        .iter()
        .map(|shift| char::from(((id >> shift) & 0x1f) as u8 + 64))
        .collect();
    let (mut model, mut serial) = (None, None);
    for descriptor in edid[54..126].chunks(18) {
        if descriptor[..3] != [0, 0, 0] {
            continue;
        }
        let text = String::from_utf8_lossy(&descriptor[5..18])
            .split('\n')
            .next()
            .unwrap_or_default()
            .trim_matches(|c: char| c.is_whitespace() || c == '\0')
            .to_string();
        if text.is_empty() {
            continue;
        }
        match descriptor[3] {
            0xfc => model = Some(text),
            0xff => serial = Some(text),
            _ => {}
        }
    }
    // The first descriptor is the preferred timing, i.e. the panel's native resolution
    let resolution = (edid[54] != 0 || edid[55] != 0).then(|| {
        let width = u32::from(edid[56]) | (u32::from(edid[58] & 0xf0) << 4);
        let height = u32::from(edid[59]) | (u32::from(edid[61] & 0xf0) << 4);
        format!("{width}x{height}")
    });
    Some(Edid { manufacturer, model, serial, resolution })
}

// Common EDID vendor codes; unknown codes are reported as-is
fn monitor_vendor(code: &str) -> String {
    match code {
        "ACR" => "Acer",
        "AOC" => "AOC",
        "AUS" => "ASUS",
        "BNQ" => "BenQ",
        "DEL" => "Dell",
        "GSM" => "LG",
        "HWP" => "HP",
        "LEN" => "Lenovo",
        "MSI" => "MSI",
        "PHL" => "Philips",
        "SAM" => "Samsung",
        "VSC" => "ViewSonic",
        other => other,
    }
    .to_string()
}

fn read_edid(device_id: &str) -> Option<Edid> {
    let key = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(format!(r"SYSTEM\CurrentControlSet\Enum\{device_id}\Device Parameters"))
        .ok()?;
    parse_edid(&key.get_raw_value("EDID").ok()?.bytes)
}

fn device_info(row: DeviceRow) -> PeripheralDevice {
    let class = row.class.unwrap_or_default().to_ascii_lowercase();
    let kind = match class.as_str() {
        CLASS_MONITOR => "monitor",
        CLASS_IMAGING => "imaging",
        _ => "unknown",
    };
    let mut device = PeripheralDevice {
        kind: kind.into(),
        name: row.name.unwrap_or_default(),
        manufacturer: row.manufacturer,
        device_id: row.device_id,
        status: row.status,
        serial: None,
        resolution: None,
    };
    if kind == "monitor" {
        let device_id = device.device_id.clone().unwrap_or_default();
        if let Some(edid) = read_edid(&device_id) {
            device.manufacturer = Some(monitor_vendor(&edid.manufacturer));
            if let Some(model) = edid.model {
                device.name = model;
            }
            device.serial = edid.serial;
            device.resolution = edid.resolution;
        }
    }
    device
}

pub fn fill(wmi: &WMIConnection, sys: &mut SystemInfo) {
    let printers = match wmi.raw_query::<PrinterRow>(
        "SELECT Name, DriverName, PortName, ServerName, Network, Shared, PrinterStatus FROM Win32_Printer",
    ) {
        Ok(rows) => Some(rows.into_iter().filter_map(printer_info).collect()),
        Err(error) => {
            tracing::warn!("collect printers: {error:#}");
            None
        }
    };

    let device_query = format!(
        "SELECT Name, ClassGuid, Manufacturer, DeviceID, Status FROM Win32_PnPEntity \
         WHERE Present = TRUE AND (ClassGuid = '{}' OR ClassGuid = '{}')",
        CLASS_MONITOR, CLASS_IMAGING,
    );
    let devices = match wmi.raw_query::<DeviceRow>(&device_query) {
        Ok(rows) => Some(rows.into_iter().map(device_info).collect()),
        Err(error) => {
            tracing::warn!("collect peripheral devices: {error:#}");
            None
        }
    };

    sys.peripherals = Some(PeripheralInventory { printers, devices });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn printer(status: Option<u16>) -> PrinterRow {
        PrinterRow {
            name: Some("printer".into()),
            driver: None,
            port: None,
            server: None,
            network: Some(true),
            shared: None,
            status,
        }
    }

    fn device(class: Option<&str>) -> DeviceRow {
        DeviceRow {
            name: Some("device".into()),
            class: class.map(str::to_string),
            manufacturer: None,
            device_id: None,
            status: None,
        }
    }

    #[test]
    fn excludes_printers_with_status_offline() {
        assert!(printer_info(printer(Some(7))).is_none());
        assert!(printer_info(printer(Some(3))).is_some());
        assert!(printer_info(printer(None)).is_some());
    }

    #[test]
    fn excludes_virtual_printer_ports() {
        let mut pdf = printer(Some(3));
        pdf.port = Some("PORTPROMPT:".into());
        assert!(printer_info(pdf).is_none());
        let mut usb = printer(Some(3));
        usb.port = Some("USB002".into());
        assert!(printer_info(usb).is_some());
    }

    #[test]
    fn parses_edid_model_and_serial() {
        let mut edid = vec![0u8; 128];
        edid[..8].copy_from_slice(&[0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0]);
        edid[8..10].copy_from_slice(&0x1e6du16.to_be_bytes()); // GSM
        edid[90 + 3] = 0xfc;
        edid[90 + 5..90 + 5 + 6].copy_from_slice(b"VG249\n");
        // 1920x1080 preferred timing: nonzero pixel clock, then width/height low and high nibbles
        edid[54..72].fill(0);
        edid[54] = 1;
        edid[56] = 0x80;
        edid[58] = 0x70;
        edid[59] = 0x38;
        edid[61] = 0x40;
        edid[72 + 3] = 0xff;
        edid[72 + 5..72 + 5 + 4].copy_from_slice(b"S123");
        let parsed = parse_edid(&edid).unwrap();
        assert_eq!(parsed.manufacturer, "GSM");
        assert_eq!(parsed.model.as_deref(), Some("VG249"));
        assert_eq!(parsed.serial.as_deref(), Some("S123"));
        assert_eq!(parsed.resolution.as_deref(), Some("1920x1080"));
        assert!(parse_edid(&[0; 128]).is_none());
    }

    #[test]
    fn maps_imaging_devices_without_claiming_they_are_scanners() {
        let info = device_info(device(Some(CLASS_IMAGING)));
        assert_eq!(info.kind, "imaging");
    }

    #[test]
    fn maps_device_class_guids_case_insensitively() {
        for (class, expected) in [
            ("{4D36E96E-E325-11CE-BFC1-08002BE10318}", "monitor"),
        ] {
            assert_eq!(device_info(device(Some(class))).kind, expected);
        }
    }

    #[test]
    fn unrecognised_or_missing_class_is_unknown() {
        assert_eq!(device_info(device(Some("{00000000-0000-0000-0000-000000000000}"))).kind, "unknown");
        assert_eq!(device_info(device(None)).kind, "unknown");
    }

    #[test]
    fn missing_name_does_not_panic() {
        let info = device_info(DeviceRow { name: None, ..device(Some(CLASS_MONITOR)) });
        assert_eq!(info.name, "");
    }
}