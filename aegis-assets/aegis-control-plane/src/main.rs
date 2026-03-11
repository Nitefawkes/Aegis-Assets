use aegis_core::{
    archive::ComplianceRegistry,
    audit::verify_audit_log,
    events::{ExtractionEvent, ExtractionEventEmitter},
    Config, EnterpriseConfig, Extractor, PluginRegistry,
};
use aegis_unity_plugin::UnityPluginFactory;
use axum::{
    extract::{Path as AxumPath, State},
    http::{Request, StatusCode},
    middleware::{self, Next},
    response::sse::{Event, KeepAlive, Sse},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::{
    convert::Infallible,
    fs,
    net::SocketAddr,
    path::{Component, Path as StdPath, PathBuf},
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
    compliance_registry: Arc<ComplianceRegistry>,
    audit_log_dir: PathBuf,
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
    ownership: Option<OwnershipVerificationRequest>,
}

#[derive(Debug, Deserialize)]
struct OwnershipVerificationRequest {
    platform: String,
    app_id: String,
    account_id: String,
}

#[derive(Debug, Serialize)]
struct ExtractResponse {
    job_id: Uuid,
    ownership_verified: bool,
}

#[derive(Debug, Serialize)]
struct AuditVerifyResponse {
    job_id: Uuid,
    verified: bool,
}

#[derive(Debug, Serialize)]
struct OwnershipVerifyResponse {
    verified: bool,
    platform: String,
    app_id: String,
    account_id: String,
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
    let compliance_registry = load_compliance_registry();
    let audit_log_dir = std::env::var("AEGIS_AUDIT_LOG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("audit-logs"));

    let state = Arc::new(AppState {
        event_tx,
        api_key,
        rate_limiter,
        plugin_registry: registry,
        compliance_registry,
        audit_log_dir,
    });

    let app = Router::new()
        .route("/events/stream", get(stream_events))
        .route("/jobs/extract", post(start_extract_job))
        .route("/audit/verify/{job_id}", get(verify_job_audit_log))
        .route("/ownership/verify", post(verify_ownership))
        .with_state(state.clone())
        .layer(ServiceBuilder::new().layer(middleware::from_fn_with_state(
            state.clone(),
            auth_rate_limit_middleware,
        )));

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    info!("Aegis control-plane listening on http://{}", addr);

    axum::serve(
        tokio::net::TcpListener::bind(addr).await.expect("bind"),
        app,
    )
    .await
    .expect("server failed");
}

fn load_compliance_registry() -> Arc<ComplianceRegistry> {
    let dir = std::env::var("AEGIS_COMPLIANCE_PROFILE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("compliance-profiles"));
    match ComplianceRegistry::load_from_directory(&dir) {
        Ok(registry) => {
            info!(
                compliance_profiles = registry.len(),
                path = %dir.display(),
                "Loaded compliance profiles"
            );
            Arc::new(registry)
        }
        Err(error) => {
            warn!(
                ?error,
                path = %dir.display(),
                "Failed to load compliance profiles; using empty registry"
            );
            Arc::new(ComplianceRegistry::new())
        }
    }
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

    let ownership_verified = verify_ownership_requirement(request.ownership.as_ref())
        .map_err(|error| (StatusCode::FORBIDDEN, error))?;
    let event_sender = state.event_tx.clone();
    let plugin_registry = state.plugin_registry.clone();
    let compliance_registry = state.compliance_registry.clone();

    validate_path(&source_path).map_err(|error| (StatusCode::BAD_REQUEST, error))?;
    validate_path(&output_dir).map_err(|error| (StatusCode::BAD_REQUEST, error))?;

    info!(
        job_id = %job_id,
        source_path = %source_path.display(),
        output_dir = %output_dir.display(),
        "Extraction job requested"
    );

    let audit_log_dir = state.audit_log_dir.clone();

    tokio::task::spawn_blocking(move || {
        let config = Config {
            enterprise_config: Some(EnterpriseConfig {
                enable_audit_logs: true,
                audit_log_dir,
                require_compliance_verification: false,
                steam_api_key: None,
                epic_api_key: None,
            }),
            ..Config::default()
        };
        let mut extractor =
            Extractor::with_registries(&plugin_registry, &compliance_registry, config);
        extractor.set_event_emitter(Arc::new(ChannelEventEmitter {
            sender: event_sender,
        }));
        if let Err(error) =
            extractor.extract_from_file_with_job_id(&source_path, &output_dir, job_id)
        {
            warn!(?error, "Extraction failed");
        }
    });

    Ok(Json(ExtractResponse {
        job_id,
        ownership_verified,
    }))
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

async fn verify_ownership(
    Json(request): Json<OwnershipVerificationRequest>,
) -> Result<Json<OwnershipVerifyResponse>, (StatusCode, String)> {
    verify_ownership_allowlist(&request).map_err(|error| (StatusCode::FORBIDDEN, error))?;

    Ok(Json(OwnershipVerifyResponse {
        verified: true,
        platform: request.platform,
        app_id: request.app_id,
        account_id: request.account_id,
    }))
}

async fn verify_job_audit_log(
    State(state): State<Arc<AppState>>,
    AxumPath(job_id): AxumPath<Uuid>,
) -> Result<Json<AuditVerifyResponse>, (StatusCode, String)> {
    verify_job_audit_files(&state.audit_log_dir, job_id).map(Json)
}

fn audit_log_paths(audit_log_dir: &StdPath, job_id: Uuid) -> (PathBuf, PathBuf) {
    (
        audit_log_dir.join(format!("extraction-{}.jsonl", job_id)),
        audit_log_dir.join(format!("extraction-{}.jsonl.blake3", job_id)),
    )
}

fn verify_job_audit_files(
    audit_log_dir: &StdPath,
    job_id: Uuid,
) -> Result<AuditVerifyResponse, (StatusCode, String)> {
    let (log_path, hash_path) = audit_log_paths(audit_log_dir, job_id);

    if !log_path.exists() || !hash_path.exists() {
        return Err((
            StatusCode::NOT_FOUND,
            format!("Audit files for job {} were not found", job_id),
        ));
    }

    verify_audit_log(&log_path, &hash_path)
        .map_err(|error| (StatusCode::UNPROCESSABLE_ENTITY, error.to_string()))?;

    Ok(AuditVerifyResponse {
        job_id,
        verified: true,
    })
}

fn verify_ownership_requirement(
    ownership: Option<&OwnershipVerificationRequest>,
) -> Result<bool, String> {
    let require_verification = std::env::var("AEGIS_REQUIRE_OWNERSHIP_VERIFICATION")
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    if !require_verification {
        return Ok(false);
    }

    let ownership = ownership
        .ok_or_else(|| "Ownership verification is required for extraction requests.".to_string())?;

    verify_ownership_allowlist(ownership)?;
    Ok(true)
}

fn verify_ownership_allowlist(ownership: &OwnershipVerificationRequest) -> Result<(), String> {
    let allowlist_path = std::env::var("AEGIS_OWNERSHIP_ALLOWLIST")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("compliance-profiles/ownership-allowlist.json"));

    let raw = fs::read_to_string(&allowlist_path).map_err(|_| {
        format!(
            "Ownership allowlist not found at {}",
            allowlist_path.display()
        )
    })?;

    let allowlist: std::collections::HashMap<String, Vec<String>> = serde_json::from_str(&raw)
        .map_err(|error| {
            format!(
                "Ownership allowlist at {} is invalid JSON: {}",
                allowlist_path.display(),
                error
            )
        })?;

    let expected = format!("{}:{}", ownership.platform.to_lowercase(), ownership.app_id);
    let owned = allowlist
        .get(&ownership.account_id)
        .map(|apps| apps.iter().any(|entry| entry == &expected))
        .unwrap_or(false);

    if owned {
        Ok(())
    } else {
        Err(format!(
            "Ownership verification failed for account '{}' and title '{}'",
            ownership.account_id, expected
        ))
    }
}

fn validate_path(path: &StdPath) -> Result<(), String> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use aegis_core::{
        audit::AuditLogger,
        events::{ExtractionEvent, ExtractionEventKind, JobState},
    };
    use chrono::Utc;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn verify_job_audit_files_returns_not_found_when_missing() {
        let dir = tempdir().expect("create temp dir");
        let job_id = Uuid::new_v4();

        let result = verify_job_audit_files(dir.path(), job_id);
        assert!(matches!(result, Err((StatusCode::NOT_FOUND, _))));
    }

    #[test]
    fn verify_job_audit_files_returns_ok_for_valid_chain() {
        let dir = tempdir().expect("create temp dir");
        let job_id = Uuid::new_v4();
        let logger = AuditLogger::new(dir.path(), job_id).expect("create logger");

        let event = ExtractionEvent {
            job_id,
            occurred_at: Utc::now(),
            kind: ExtractionEventKind::JobStateChange {
                state: JobState::Queued,
                message: Some("queued".to_string()),
            },
        };
        logger.log_event(&event).expect("log event");

        let result = verify_job_audit_files(dir.path(), job_id).expect("verification should pass");
        assert!(result.verified);
        assert_eq!(result.job_id, job_id);
    }

    #[test]
    fn verify_job_audit_files_returns_unprocessable_for_tamper() {
        let dir = tempdir().expect("create temp dir");
        let job_id = Uuid::new_v4();
        let logger = AuditLogger::new(dir.path(), job_id).expect("create logger");

        let event = ExtractionEvent {
            job_id,
            occurred_at: Utc::now(),
            kind: ExtractionEventKind::JobStateChange {
                state: JobState::Queued,
                message: Some("queued".to_string()),
            },
        };
        logger.log_event(&event).expect("log event");

        let hash_path = logger.hash_path().to_path_buf();
        fs::write(hash_path, "0 deadbeef\n").expect("tamper hash file");

        let result = verify_job_audit_files(dir.path(), job_id);
        assert!(matches!(result, Err((StatusCode::UNPROCESSABLE_ENTITY, _))));
    }

    #[test]
    fn ownership_verification_skips_when_not_required() {
        std::env::remove_var("AEGIS_REQUIRE_OWNERSHIP_VERIFICATION");
        let result = verify_ownership_requirement(None).expect("should not fail");
        assert!(!result);
    }

    #[test]
    fn ownership_verification_accepts_allowlisted_title() {
        let dir = tempdir().expect("create temp dir");
        let path = dir.path().join("allowlist.json");
        fs::write(&path, r#"{"acct-1":["steam:570"]}"#).expect("write allowlist");

        std::env::set_var("AEGIS_REQUIRE_OWNERSHIP_VERIFICATION", "true");
        std::env::set_var("AEGIS_OWNERSHIP_ALLOWLIST", &path);

        let request = OwnershipVerificationRequest {
            platform: "steam".to_string(),
            app_id: "570".to_string(),
            account_id: "acct-1".to_string(),
        };

        let result =
            verify_ownership_requirement(Some(&request)).expect("verification should pass");
        assert!(result);
    }

    #[test]
    fn ownership_verification_rejects_missing_allowlist() {
        std::env::set_var("AEGIS_REQUIRE_OWNERSHIP_VERIFICATION", "true");
        std::env::set_var("AEGIS_OWNERSHIP_ALLOWLIST", "does-not-exist.json");

        let request = OwnershipVerificationRequest {
            platform: "steam".to_string(),
            app_id: "570".to_string(),
            account_id: "acct-1".to_string(),
        };

        let result = verify_ownership_requirement(Some(&request));
        assert!(result.is_err());
    }

    #[test]
    fn ownership_verification_rejects_missing_title() {
        let dir = tempdir().expect("create temp dir");
        let path = dir.path().join("allowlist.json");
        fs::write(&path, r#"{"acct-1":["steam:730"]}"#).expect("write allowlist");

        std::env::set_var("AEGIS_REQUIRE_OWNERSHIP_VERIFICATION", "true");
        std::env::set_var("AEGIS_OWNERSHIP_ALLOWLIST", &path);

        let request = OwnershipVerificationRequest {
            platform: "steam".to_string(),
            app_id: "570".to_string(),
            account_id: "acct-1".to_string(),
        };

        let result = verify_ownership_requirement(Some(&request));
        assert!(result.is_err());
    }
}
