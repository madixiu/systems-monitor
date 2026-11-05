use axum::extract::{Query, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use futures::stream::Stream;
use serde::Deserialize;
use std::convert::Infallible;
use std::time::Duration;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

use crate::auth::verify_api_key;
use crate::error::AppError;
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct StreamParams { pub key: Option<String> }

pub async fn sse(
    State(state): State<AppState>,
    Query(p): Query<StreamParams>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, AppError> {
    let key = p.key.ok_or(AppError::Unauthorized("missing ?key="))?;
    verify_api_key(&state.pool, &key).await?;

    let rx = state.tx.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|msg| match msg {
        Ok(ev) => Some(Ok(Event::default().json_data(&ev).unwrap_or_default())),
        Err(_) => None,
    });

    Ok(Sse::new(stream).keep_alive(
        KeepAlive::new().interval(Duration::from_secs(15)).text("keepalive"),
    ))
}
