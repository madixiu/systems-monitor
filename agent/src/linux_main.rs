use anyhow::{Context, Result};
use chrono::Utc;
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
    let mut retry_at: Option<std::time::Instant> = None;
    let mut retry_delay = Duration::from_secs(60);

    loop {
        if let Err(e) = tick(
            &client,
            &mut st,
            report_dur,
            &mut retry_at,
            &mut retry_delay,
        ).await {
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
    report_dur: Duration,
    retry_at: &mut Option<std::time::Instant>,
    retry_delay: &mut Duration,
) -> Result<()> {
    let hb = client.heartbeat(st).await?;

    let due = st.last_report_at
        .map(|t| Utc::now().signed_duration_since(t).to_std().unwrap_or_default() >= report_dur)
        .unwrap_or(false);

    if hb.full_report || due {
        if retry_at.is_some_and(|at| std::time::Instant::now() < at) {
            return Ok(());
        }

        let mut report: Report = match collect_all(st.agent_id).await {
            Ok(report) => report,
            Err(error) => {
                schedule_retry(retry_at, retry_delay);
                return Err(error);
            }
        };
        report.refresh_generation = hb.refresh_generation;

        if let Err(error) = client.ingest(st, &report).await {
            schedule_retry(retry_at, retry_delay);
            return Err(error);
        }

        st.last_report_at = Some(Utc::now());
        st.save()?;
        *retry_at = None;
        *retry_delay = Duration::from_secs(60);
        info!(
            host = %report.system.computer_name,
            ips = ?report.system.ip_addresses,
            "report sent"
        );
    }

    fn schedule_retry(retry_at: &mut Option<std::time::Instant>, retry_delay: &mut Duration) {
        *retry_at = Some(std::time::Instant::now() + *retry_delay);
        *retry_delay = std::cmp::min(*retry_delay + *retry_delay, Duration::from_secs(3600));
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