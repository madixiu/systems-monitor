#[cfg(target_os = "windows")]
#[path = "windows_main.rs"]
mod platform;

#[cfg(target_os = "linux")]
#[path = "linux_main.rs"]
mod platform;

mod config;
mod report;
mod state;
mod types;

#[cfg(target_os = "windows")]
#[path = "collect/mod.rs"]
pub mod collect;

#[cfg(target_os = "linux")]
#[path = "linux/mod.rs"]
pub mod collect;

fn main() -> anyhow::Result<()> {
    platform::run()
}
