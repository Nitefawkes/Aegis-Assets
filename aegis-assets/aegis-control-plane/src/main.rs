use aegis_core::{
    archive::ComplianceRegistry,
    events::{ExtractionEvent, ExtractionEventEmitter},
    Extractor,
};
use axum::{
    extract::State,
    http::StatusCode,
    response::sse::{Event, KeepAlive, Sse},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::{convert::Infallible, net::SocketAddr, path::PathBuf, sync::Arc, time::Duration};
use tokio::sync::broadcast;
use tokio_stream::{
    wrappers::{errors::BroadcastStreamRecvError, BroadcastStream},
    StreamExt,
};
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

#[derive(Clone)]
struct AppState {
    event_tx: broadcast::Sender<ExtractionEvent>,
}

#[derive(Clone)]
struct ChannelEventEmitter {
    sender: broadcast::Sender<ExtractionEvent>,
}

impl ExtractionEventEmitter for ChannelEventEmitter {
    fn emit(&self, event: ExtractionEvent) {
        if let Err(error) = self.sender.send(event) {
            warn!(?error, "Failed to broadcast extraction event");
        }
    }
}

#[derive(Debug, Deserialize)]
struct ExtractRequest {
    source_path: String,
    output_dir: String,
}

#[derive(Debug, Serialize)]
struct ExtractResponse {
    job_id: Uuid,
}

#[tokio::main]
async fn main() {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("aegis_control_plane=info,aegis_core=info"));
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(false)
        .init();

    let (event_tx, _) = broadcast::channel(256);
    let state = Arc::new(AppState { event_tx });

    let app = Router::new()
        .route("/events/stream", get(stream_events))
        .route("/jobs/extract", post(start_extract_job))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    info!("Aegis control-plane listening on http://{}", addr);

    axum::serve(
        tokio::net::TcpListener::bind(addr).await.expect("bind"),
        app,
    )
    .await
    .expect("server failed");
}

async fn stream_events(
    State(state): State<Arc<AppState>>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let receiver = state.event_tx.subscribe();
    let stream = BroadcastStream::new(receiver).filter_map(|message| match message {
        Ok(event) => {
            let payload = serde_json::to_string(&event).unwrap_or_else(|_| "{}".to_string());
            Some(Ok(Event::default().event("extraction").data(payload)))
        }
        Err(BroadcastStreamRecvError::Lagged(_)) => None,
    });

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    )
}

async fn start_extract_job(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ExtractRequest>,
) -> Result<Json<ExtractResponse>, (StatusCode, String)> {
    let job_id = Uuid::new_v4();
    let source_path = PathBuf::from(request.source_path);
    let output_dir = PathBuf::from(request.output_dir);
    let event_sender = state.event_tx.clone();

    tokio::task::spawn_blocking(move || {
        let compliance = ComplianceRegistry::new();
        let mut extractor = Extractor::new(compliance);
        extractor.set_event_emitter(Arc::new(ChannelEventEmitter {
            sender: event_sender,
        }));
        if let Err(error) =
            extractor.extract_from_file_with_job_id(&source_path, &output_dir, job_id)
        {
            warn!(?error, "Extraction failed");
        }
    });

    Ok(Json(ExtractResponse { job_id }))
}
