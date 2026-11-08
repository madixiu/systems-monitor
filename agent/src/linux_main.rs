use anyhow::{Context, Result};
use std::time::Duration;
use tokio::sync::watch;
use tracing::{error, info};
use uuid::Uuid;

use crate::collect;
use crate::config::Config;
use crate::report::ReportClient;
use crate::state::State;
use crate::types::Report;

pub fn run() -> Result<()> {
    init_logging();

    let rt = tokio::runtime::Runtime::new()?;
    let (tx, rx) = watch::channel(false);
    drop(tx); // no shutdown signal in simple mode — Ctrl+C or systemd stop

    rt.block_on(async move {
        info!("running in foreground mode (systemd managed)");
        main_loop(rx).await
    })
}

async fn main_loop(mut shutdown_rx: watch::Receiver<bool>) -> Result<()> {
    let cfg = Config::load().context("load config")?;
    let client = ReportClient::new(&cfg)?;
    let mut st = State::load_or_enroll(&cfg, &client).await.context("enroll")?;

    info!(agent_id = %st.agent_id, "agent ready");

    let hb_dur = Duration::from_secs(cfg.interval.heartbeat_secs.max(10));
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
        None => true,
        Some(t) => t.elapsed() >= report_dur,
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

fn init_logging() {
    let level = crate::config::Config::load()
        .map(|c| c.log.level)
        .unwrap_or_else(|_| "info".into());
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&level)),
        )
        .with_ansi(false)
        .init();
}