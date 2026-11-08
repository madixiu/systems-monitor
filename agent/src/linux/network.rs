use anyhow::Result;
use nix::ifaddrs::getifaddrs;
use nix::sys::socket::{AddressFamily, SockaddrLike};

use crate::types::SystemInfo;

pub fn fill(sys: &mut SystemInfo) -> Result<()> {
    let mut ips: Vec<String> = Vec::new();
    let mut macs: Vec<String> = Vec::new();

    for ifaddr in getifaddrs()? {
        let name = ifaddr.interface_name.as_str();
        if name == "lo" || name.starts_with("docker") || name.starts_with("veth") || name.starts_with("br-") {
            continue;
        }

        // MAC
            // MAC
        if let Some(addr) = &ifaddr.address {
            if let Some(link) = addr.as_link_addr() {
                if let Some(mac_bytes) = link.addr() {
                    if mac_bytes.len() == 6 && mac_bytes.iter().any(|b| *b != 0) {
                        let mac = mac_bytes.iter()
                            .map(|b| format!("{:02X}", b))
                            .collect::<Vec<_>>()
                            .join(":");
                        if !macs.contains(&mac) {
                            macs.push(mac);
                        }
                    }
                }
            }
        }

        // IP
        if let Some(addr) = &ifaddr.address {
            if addr.family() == Some(AddressFamily::Inet) {
                if let Some(inet) = addr.as_sockaddr_in() {
                    let ip = std::net::Ipv4Addr::from(inet.ip());
                    let s = ip.to_string();
                    if !s.starts_with("127.") && !s.starts_with("169.254.") && !ips.contains(&s) {
                        ips.push(s);
                    }
                }
            }
        }
    }

    sys.ip_addresses = ips;
    sys.mac_addresses = macs;
    Ok(())
}