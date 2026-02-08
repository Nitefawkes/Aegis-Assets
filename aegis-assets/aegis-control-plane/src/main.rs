use aegis_core::{
    archive::ComplianceRegistry,
    events::{ExtractionEvent, ExtractionEventEmitter},
    Config, Extractor, PluginRegistry,
};
use aegis_unity_plugin::UnityPluginFactory;
use axum::{
    extract::State,
    http::{Request, StatusCode},
    middleware::{self, Next},
    response::sse::{Event, KeepAlive, Sse},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::{
    convert::Infallible,
    net::SocketAddr,
    path::{Component, Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::broadcast;
use tokio_stream::{
    wrappers::{errors::BroadcastStreamRecvError, BroadcastStream},
    StreamExt,
};
use tower::ServiceBuilder;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

#[derive(Clone)]
struct AppState {
    event_tx: broadcast::Sender<ExtractionEvent>,
    api_key: Option<String>,
    rate_limiter: RateLimiter,
    plugin_registry: Arc<PluginRegistry>,
}

#[derive(Clone)]
struct RateLimiter {
    max_requests: u32,
    window: Duration,
    state: Arc<tokio::sync::Mutex<RateLimitState>>,
}

struct RateLimitState {
    window_start: Instant,
    request_count: u32,
}

impl RateLimiter {
    fn new(max_requests: u32, window: Duration) -> Self {
        Self {
            max_requests,
            window,
            state: Arc::new(tokio::sync::Mutex::new(RateLimitState {
                window_start: Instant::now(),
                request_count: 0,
            })),
        }
    }

    async fn allow(&self) -> bool {
        let mut state = self.state.lock().await;
        let now = Instant::now();
        if now.duration_since(state.window_start) >= self.window {
            state.window_start = now;
            state.request_count = 0;
        }

        if state.request_count >= self.max_requests {
            return false;
        }

        state.request_count += 1;
        true
    }
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
    let api_key = std::env::var("AEGIS_CONTROL_PLANE_API_KEY").ok();
    if api_key.is_none() {
        warn!("AEGIS_CONTROL_PLANE_API_KEY is not set; control-plane requests will be rejected");
    }

    let max_requests = std::env::var("AEGIS_CONTROL_PLANE_RATE_LIMIT")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(60);
    let rate_limiter = RateLimiter::new(max_requests, Duration::from_secs(60));

    let mut registry = PluginRegistry::new();
    registry.register_plugin(Box::new(UnityPluginFactory));
    let registry = Arc::new(registry);

    let state = Arc::new(AppState {
        event_tx,
        api_key,
        rate_limiter,
        plugin_registry: registry,
    });

    let app = Router::new()
        .route("/events/stream", get(stream_events))
        .route("/jobs/extract", post(start_extract_job))
        .with_state(state.clone())
        .layer(
            ServiceBuilder::new().layer(middleware::from_fn_with_state(
                state.clone(),
                auth_rate_limit_middleware,
            )),
        );

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
    let plugin_registry = state.plugin_registry.clone();

    validate_path(&source_path).map_err(|error| (StatusCode::BAD_REQUEST, error))?;
    validate_path(&output_dir).map_err(|error| (StatusCode::BAD_REQUEST, error))?;

    info!(
        job_id = %job_id,
        source_path = %source_path.display(),
        output_dir = %output_dir.display(),
        "Extraction job requested"
    );

    tokio::task::spawn_blocking(move || {
        let compliance = ComplianceRegistry::new();
        let mut extractor =
            Extractor::with_registries(&plugin_registry, &compliance, Config::default());
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

async fn auth_rate_limit_middleware(
    State(state): State<Arc<AppState>>,
    req: Request<axum::body::Body>,
    next: Next,
) -> Result<axum::response::Response, StatusCode> {
    if !state.rate_limiter.allow().await {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    let required_key = state
        .api_key
        .as_deref()
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    let provided_key = req
        .headers()
        .get("x-api-key")
        .and_then(|value| value.to_str().ok());

    if provided_key != Some(required_key) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    Ok(next.run(req).await)
}

fn validate_path(path: &Path) -> Result<(), String> {
    if path.as_os_str().is_empty() {
        return Err("Path must not be empty.".to_string());
    }

    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err("Path must not contain parent directory traversal.".to_string());
    }

    Ok(())
}
