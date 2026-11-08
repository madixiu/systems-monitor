use crate::collect;
// mod collect;
// mod config;
// mod report;
// mod state;
// mod types;

use anyhow::{Context, Result};
use std::ffi::OsString;
use std::time::Duration;
use tokio::sync::watch;
use tracing::{error, info};
use uuid::Uuid;

use crate::config::Config;
use crate::report::ReportClient;
use crate::state::State;
use crate::types::Report;

use windows_service::{
    define_windows_service,
    service::{ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus, ServiceType},
    service_control_handler::{self, ServiceControlHandlerResult},
    service_dispatcher,
};

const SERVICE_NAME: &str = "PakMonitorAgent";
define_windows_service!(ffi_service_main, service_main);

// ---------- entry ----------

pub fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let console = args.iter().any(|a| a == "--console");

    if console {
        init_logging_console();
        info!("running in console mode");
        let rt = tokio::runtime::Runtime::new()?;
        let (_tx, rx) = watch::channel(false);
        rt.block_on(main_loop(rx))?;
        Ok(())
    } else {
        service_dispatcher::start(SERVICE_NAME, ffi_service_main)
            .map_err(|e| anyhow::anyhow!("service_dispatcher::start: {e}"))
    }
}

fn service_main(_args: Vec<OsString>) {
    // Init logging first — anything below can log.
    if let Err(e) = init_logging_file() {
        // Can't log yet; write to disk manually as last resort.
        let _ = std::fs::write(
            r"C:\ProgramData\PakMonitor\startup-error.txt",
            format!("{:#}", e),
        );
        return;
    }
    if let Err(e) = run_service() {
        error!("service exited with error: {e:#}");
    }
}

// ---------- service plumbing ----------

fn run_service() -> Result<()> {
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    let event_handler = move |control| -> ServiceControlHandlerResult {
        match control {
            ServiceControl::Stop | ServiceControl::Shutdown => {
                let _ = shutdown_tx.send(true);
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    let status_handle = service_control_handler::register(SERVICE_NAME, event_handler)?;

    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    let rt = tokio::runtime::Runtime::new()?;
    let result = rt.block_on(main_loop(shutdown_rx));

    let exit_code = match &result {
        Ok(())  => ServiceExitCode::Win32(0),
        Err(_)  => ServiceExitCode::ServiceSpecific(1),
    };
    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code,
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    result
}

// ---------- main loop ----------

async fn main_loop(mut shutdown_rx: watch::Receiver<bool>) -> Result<()> {
    let cfg = Config::load().context("load config")?;
    let client = ReportClient::new(&cfg)?;
    let mut st = State::load_or_enroll(&cfg, &client).await.context("enroll")?;

    info!(agent_id = %st.agent_id, "agent ready");

    let hb_dur     = Duration::from_secs(cfg.interval.heartbeat_secs.max(10));
    let report_dur = Duration::from_secs(cfg.interval.report_secs.max(60));
    let mut last_report: Option<std::time::Instant> = None;

    loop {
        if let Err(e) = tick(&client, &mut st, &mut last_report, report_dur).await {
            error!("tick: {e:#}");
        }

        tokio::select! {
            _ = tokio::time::sleep(hb_dur) => {}
            _ = shutdown_rx.changed() => {
                if *shutdown_rx.borrow() { break; }
            }
        }
    }

    info!("shutdown");
    Ok(())
}

async fn tick(
    client: &ReportClient,
    st: &mut State,
    last_report: &mut Option<std::time::Instant>,
    report_dur: Duration,
) -> Result<()> {
    let hb = client.heartbeat(st).await?;

    let due = match *last_report {
        None      => true,
        Some(t)   => t.elapsed() >= report_dur,
    };

    if hb.full_report || due {
        let report: Report = collect_all(st.agent_id).await?;
        client.ingest(st, &report).await?;
        *last_report = Some(std::time::Instant::now());
        info!(
            host = %report.system.computer_name,
            ips = ?report.system.ip_addresses,
            "report sent"
        );
    }
    Ok(())
}

async fn collect_all(agent_id: Uuid) -> Result<Report> {
    tokio::task::spawn_blocking(move || collect::collect_sync(agent_id))
        .await
        .context("spawn_blocking")?
}

// ---------- logging ----------
fn init_logging_console() {
    let level = Config::load().map(|c| c.log.level).unwrap_or_else(|_| "info".into());
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&level)),
        )
        .init();
}

fn init_logging_file() -> Result<()> {
    let dir = std::path::Path::new(crate::config::CONFIG_DIR);
    std::fs::create_dir_all(dir)?;
    let appender = tracing_appender::rolling::daily(dir, "agent.log");
    let (nb, _guard) = tracing_appender::non_blocking(appender);
    std::mem::forget(_guard);

    let level = Config::load().map(|c| c.log.level).unwrap_or_else(|_| "info".into());
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&level)),
        )
        .with_writer(nb)
        .with_ansi(false)
        .init();

    Ok(())
}