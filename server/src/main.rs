mod auth;
mod broadcast;
mod config;
mod db;
mod error;
mod routes;

use anyhow::Result;
use axum::routing::{get, post};
use axum::Router;
use sqlx::PgPool;
use tokio::sync::broadcast as tokio_broadcast;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use broadcast::SystemEvent;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub tx: tokio_broadcast::Sender<SystemEvent>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cfg = config::Config::load()?;

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&cfg.log_level)),
        )
        .init();

    tracing::info!(bind = %cfg.bind, "starting pak-monitor-server");

    let pool = db::connect(&cfg.database_url).await?;
    tracing::info!("database connected");

    db::migrate(&pool).await?;
    tracing::info!("migrations applied");

    db::seed_api_key_if_missing(&pool).await?;
    db::seed_enrollment_secret_if_missing(&pool).await?;

    let (tx, _) = tokio_broadcast::channel::<SystemEvent>(1024);
    let state = AppState { pool, tx };

    let app = Router::new()
        .route("/api/v1/agent/enroll",    post(routes::agent::enroll))
        .route("/api/v1/agent/heartbeat", post(routes::agent::heartbeat))
        .route("/api/v1/agent/ingest",    post(routes::agent::ingest))
        .route("/api/v1/systems",         get(routes::dashboard::list))
        .route("/api/v1/systems/:id",     get(routes::dashboard::get_one))
        .route("/api/v1/systems/:id/force-refresh", post(routes::dashboard::force_refresh))
        .route("/api/v1/systems/:id/maintenance",   post(routes::dashboard::set_maintenance))
        .route("/api/v1/systems/:id/notes",
               get(routes::dashboard::list_notes).post(routes::dashboard::add_note))
        .route("/api/v1/stats",           get(routes::dashboard::stats))
        .route("/api/v1/settings",
               get(routes::dashboard::get_settings).put(routes::dashboard::put_settings))
        .route("/api/v1/stream",          get(routes::stream::sse))
        .route("/api/v1/export",          get(routes::export::export))
        .route("/health",                 get(|| async { "ok" }))
        .with_state(state)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind(&cfg.bind).await?;
    tracing::info!("listening on {}", cfg.bind);
    axum::serve(listener, app).await?;
    Ok(())
}
