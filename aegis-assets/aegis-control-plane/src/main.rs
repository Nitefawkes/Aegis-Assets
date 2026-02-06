use aegis_core::archive::ComplianceProfile;
use anyhow::{Context, Result};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{any::AnyPoolOptions, AnyPool, Row};
use std::{
    collections::BTreeMap,
    env,
    net::SocketAddr,
    path::{Path as FsPath, PathBuf},
    sync::Arc,
};
use tracing::{info, warn};
use uuid::Uuid;

#[derive(Clone)]
struct AppState {
    db: AnyPool,
    db_flavor: DbFlavor,
    profiles_dir: PathBuf,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum JobStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Clone, Copy, Debug)]
enum DbFlavor {
    Postgres,
    Sqlite,
}

#[derive(Debug, Serialize, Deserialize)]
struct Job {
    id: Uuid,
    status: JobStatus,
    payload: serde_json::Value,
    result: Option<serde_json::Value>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct CreateJobRequest {
    payload: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
struct AuditRecord {
    id: Uuid,
    event_type: String,
    actor: Option<String>,
    payload: serde_json::Value,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct CreateAuditRequest {
    event_type: String,
    actor: Option<String>,
    payload: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct AuditQuery {
    event_type: Option<String>,
    limit: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ComplianceProfileResource {
    id: String,
    profile: ComplianceProfile,
}

#[derive(Debug, Deserialize)]
struct ComplianceProfilePayload {
    id: String,
    profile: ComplianceProfile,
}

#[derive(Debug, Serialize)]
struct PluginMetadata {
    name: String,
    version: String,
    description: String,
    keywords: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let database_url =
        env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://aegis-control-plane.db".to_string());
    let profiles_dir =
        env::var("PROFILES_DIR").unwrap_or_else(|_| "compliance-profiles".to_string());
    let bind_addr = env::var("CONTROL_PLANE_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());

    let db_flavor = db_flavor_from_url(&database_url)?;
    let db = AnyPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .context("Failed to connect to database")?;
    let profiles_dir = PathBuf::from(profiles_dir);
    if !profiles_dir.exists() {
        std::fs::create_dir_all(&profiles_dir)
            .with_context(|| format!("Failed to create profiles dir {}", profiles_dir.display()))?;
    }

    initialize_schema(&db, db_flavor).await?;

    let state = Arc::new(AppState {
        db,
        db_flavor,
        profiles_dir,
    });

    let app = Router::new()
        .route("/jobs", post(create_job))
        .route("/jobs/:id", get(get_job))
        .route("/jobs/:id/cancel", post(cancel_job))
        .route("/jobs/:id/results", get(get_job_results))
        .route(
            "/compliance-profiles",
            get(list_profiles).post(create_profile),
        )
        .route(
            "/compliance-profiles/:id",
            get(get_profile).put(update_profile).delete(delete_profile),
        )
        .route("/plugins", get(list_plugins))
        .route("/audits", get(list_audits).post(create_audit))
        .with_state(state);

    let addr: SocketAddr = bind_addr.parse().context("Invalid CONTROL_PLANE_ADDR")?;
    info!("control-plane listening on {}", addr);

    axum::serve(tokio::net::TcpListener::bind(addr).await?, app).await?;

    Ok(())
}

async fn initialize_schema(pool: &AnyPool, _db_flavor: DbFlavor) -> Result<()> {
    let jobs_table = "CREATE TABLE IF NOT EXISTS jobs (
            id TEXT PRIMARY KEY,
            status TEXT NOT NULL,
            payload TEXT NOT NULL,
            result TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );";

    let audits_table = "CREATE TABLE IF NOT EXISTS audits (
            id TEXT PRIMARY KEY,
            event_type TEXT NOT NULL,
            actor TEXT,
            payload TEXT NOT NULL,
            created_at TEXT NOT NULL
        );";

    sqlx::query(jobs_table)
        .execute(pool)
        .await
        .context("Failed to create jobs table")?;
    sqlx::query(audits_table)
        .execute(pool)
        .await
        .context("Failed to create audits table")?;

    Ok(())
}

async fn create_job(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateJobRequest>,
) -> Result<(StatusCode, Json<Job>), (StatusCode, String)> {
    let job = Job {
        id: Uuid::new_v4(),
        status: JobStatus::Pending,
        payload: payload.payload.unwrap_or_else(|| serde_json::json!({})),
        result: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    insert_job(&state, &job).await.map_err(internal_error)?;

    Ok((StatusCode::CREATED, Json(job)))
}

async fn get_job(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Job>, (StatusCode, String)> {
    let job = fetch_job(&state, &id).await.map_err(internal_error)?;
    job.map(Json)
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Job not found".to_string()))
}

async fn cancel_job(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Job>, (StatusCode, String)> {
    let job = fetch_job(&state, &id).await.map_err(internal_error)?;
    let mut job = job.ok_or_else(|| (StatusCode::NOT_FOUND, "Job not found".to_string()))?;

    job.status = JobStatus::Cancelled;
    job.updated_at = Utc::now();

    update_job(&state, &job).await.map_err(internal_error)?;

    Ok(Json(job))
}

async fn get_job_results(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let job = fetch_job(&state, &id).await.map_err(internal_error)?;
    let job = job.ok_or_else(|| (StatusCode::NOT_FOUND, "Job not found".to_string()))?;

    match job.result {
        Some(result) => Ok(Json(result)),
        None => Err((StatusCode::NOT_FOUND, "Job has no results".to_string())),
    }
}

async fn list_profiles(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<ComplianceProfileResource>>, (StatusCode, String)> {
    let profiles = load_profiles(&state.profiles_dir).map_err(internal_error)?;
    Ok(Json(profiles))
}

async fn get_profile(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<ComplianceProfileResource>, (StatusCode, String)> {
    let path = profile_path(&state.profiles_dir, &id);
    if !path.exists() {
        return Err((StatusCode::NOT_FOUND, "Profile not found".to_string()));
    }

    let profile = read_profile(&path).map_err(internal_error)?;
    Ok(Json(ComplianceProfileResource { id, profile }))
}

async fn create_profile(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ComplianceProfilePayload>,
) -> Result<(StatusCode, Json<ComplianceProfileResource>), (StatusCode, String)> {
    let path = profile_path(&state.profiles_dir, &payload.id);
    if path.exists() {
        return Err((StatusCode::CONFLICT, "Profile already exists".to_string()));
    }

    write_profile(&path, &payload.profile).map_err(internal_error)?;

    Ok((
        StatusCode::CREATED,
        Json(ComplianceProfileResource {
            id: payload.id,
            profile: payload.profile,
        }),
    ))
}

async fn update_profile(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<ComplianceProfilePayload>,
) -> Result<Json<ComplianceProfileResource>, (StatusCode, String)> {
    if id != payload.id {
        return Err((StatusCode::BAD_REQUEST, "Profile id mismatch".to_string()));
    }

    let path = profile_path(&state.profiles_dir, &id);
    if !path.exists() {
        return Err((StatusCode::NOT_FOUND, "Profile not found".to_string()));
    }

    write_profile(&path, &payload.profile).map_err(internal_error)?;

    Ok(Json(ComplianceProfileResource {
        id: payload.id,
        profile: payload.profile,
    }))
}

async fn delete_profile(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let path = profile_path(&state.profiles_dir, &id);
    if !path.exists() {
        return Err((StatusCode::NOT_FOUND, "Profile not found".to_string()));
    }

    std::fs::remove_file(&path)
        .with_context(|| format!("Failed to delete profile {}", id))
        .map_err(internal_error)?;

    Ok(StatusCode::NO_CONTENT)
}

async fn list_plugins() -> Json<Vec<PluginMetadata>> {
    let plugins = vec![PluginMetadata {
        name: "aegis-unity-plugin".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        description: "Unity engine plugin for Aegis-Assets".to_string(),
        keywords: vec![
            "unity".to_string(),
            "game".to_string(),
            "assets".to_string(),
        ],
    }];

    Json(plugins)
}

async fn create_audit(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateAuditRequest>,
) -> Result<(StatusCode, Json<AuditRecord>), (StatusCode, String)> {
    let record = AuditRecord {
        id: Uuid::new_v4(),
        event_type: payload.event_type,
        actor: payload.actor,
        payload: payload.payload,
        created_at: Utc::now(),
    };

    insert_audit(&state, &record)
        .await
        .map_err(internal_error)?;

    Ok((StatusCode::CREATED, Json(record)))
}

async fn list_audits(
    State(state): State<Arc<AppState>>,
    Query(query): Query<AuditQuery>,
) -> Result<Json<Vec<AuditRecord>>, (StatusCode, String)> {
    let records = fetch_audits(&state, query).await.map_err(internal_error)?;
    Ok(Json(records))
}

fn profile_path(profiles_dir: &FsPath, id: &str) -> PathBuf {
    profiles_dir.join(format!("{}.yaml", id))
}

fn read_profile(path: &FsPath) -> Result<ComplianceProfile> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read profile {}", path.display()))?;
    let profile = serde_yaml::from_str(&content)
        .with_context(|| format!("Failed to parse profile {}", path.display()))?;
    Ok(profile)
}

fn write_profile(path: &FsPath, profile: &ComplianceProfile) -> Result<()> {
    let yaml = serde_yaml::to_string(profile).context("Failed to serialize profile")?;
    std::fs::write(path, yaml)
        .with_context(|| format!("Failed to write profile {}", path.display()))?;
    Ok(())
}

fn load_profiles(profiles_dir: &FsPath) -> Result<Vec<ComplianceProfileResource>> {
    let mut profiles = Vec::new();
    let mut errors = BTreeMap::new();

    for entry in std::fs::read_dir(profiles_dir)
        .with_context(|| format!("Failed to read profiles dir {}", profiles_dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if !path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("yaml") || ext.eq_ignore_ascii_case("yml"))
            .unwrap_or(false)
        {
            continue;
        }

        let id = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("unknown")
            .to_string();

        match read_profile(&path) {
            Ok(profile) => profiles.push(ComplianceProfileResource { id, profile }),
            Err(err) => {
                warn!("Failed to load profile {}: {err}", path.display());
                errors.insert(path.display().to_string(), err.to_string());
            }
        }
    }

    if !errors.is_empty() {
        warn!("{} profiles failed to load", errors.len());
    }

    Ok(profiles)
}

async fn insert_job(state: &AppState, job: &Job) -> Result<()> {
    let payload = serde_json::to_string(&job.payload).context("Failed to serialize payload")?;
    let result = match &job.result {
        Some(value) => Some(serde_json::to_string(value).context("Failed to serialize result")?),
        None => None,
    };
    let created_at = job.created_at.to_rfc3339();
    let updated_at = job.updated_at.to_rfc3339();

    match state.db_flavor {
        DbFlavor::Postgres => {
            sqlx::query(
                "INSERT INTO jobs (id, status, payload, result, created_at, updated_at)
                 VALUES ($1, $2, $3, $4, $5, $6)",
            )
            .bind(job.id.to_string())
            .bind(format!("{:?}", job.status).to_lowercase())
            .bind(payload)
            .bind(result)
            .bind(created_at)
            .bind(updated_at)
            .execute(&state.db)
            .await
            .context("Failed to insert job")?;
        }
        DbFlavor::Sqlite => {
            sqlx::query(
                "INSERT INTO jobs (id, status, payload, result, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(job.id.to_string())
            .bind(format!("{:?}", job.status).to_lowercase())
            .bind(payload)
            .bind(result)
            .bind(created_at)
            .bind(updated_at)
            .execute(&state.db)
            .await
            .context("Failed to insert job")?;
        }
    }

    Ok(())
}

async fn update_job(state: &AppState, job: &Job) -> Result<()> {
    let payload = serde_json::to_string(&job.payload).context("Failed to serialize payload")?;
    let result = match &job.result {
        Some(value) => Some(serde_json::to_string(value).context("Failed to serialize result")?),
        None => None,
    };
    let updated_at = job.updated_at.to_rfc3339();

    match state.db_flavor {
        DbFlavor::Postgres => {
            sqlx::query(
                "UPDATE jobs SET status = $1, payload = $2, result = $3, updated_at = $4 WHERE id = $5",
            )
            .bind(format!("{:?}", job.status).to_lowercase())
            .bind(payload)
            .bind(result)
            .bind(updated_at)
            .bind(job.id.to_string())
            .execute(&state.db)
            .await
            .context("Failed to update job")?;
        }
        DbFlavor::Sqlite => {
            sqlx::query(
                "UPDATE jobs SET status = ?, payload = ?, result = ?, updated_at = ? WHERE id = ?",
            )
            .bind(format!("{:?}", job.status).to_lowercase())
            .bind(payload)
            .bind(result)
            .bind(updated_at)
            .bind(job.id.to_string())
            .execute(&state.db)
            .await
            .context("Failed to update job")?;
        }
    }

    Ok(())
}

async fn fetch_job(state: &AppState, id: &str) -> Result<Option<Job>> {
    let row = match state.db_flavor {
        DbFlavor::Postgres => sqlx::query(
            "SELECT id, status, payload, result, created_at, updated_at FROM jobs WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&state.db)
        .await
        .context("Failed to fetch job")?,
        DbFlavor::Sqlite => sqlx::query(
            "SELECT id, status, payload, result, created_at, updated_at FROM jobs WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&state.db)
        .await
        .context("Failed to fetch job")?,
    };

    Ok(row.map(map_job_row).transpose()?)
}

fn map_job_row(row: sqlx::any::AnyRow) -> Result<Job> {
    let id: String = row.get("id");
    let status: String = row.get("status");
    let payload: String = row.get("payload");
    let result: Option<String> = row.get("result");
    let created_at: String = row.get("created_at");
    let updated_at: String = row.get("updated_at");

    let status = match status.as_str() {
        "pending" => JobStatus::Pending,
        "running" => JobStatus::Running,
        "completed" => JobStatus::Completed,
        "failed" => JobStatus::Failed,
        "cancelled" => JobStatus::Cancelled,
        other => {
            warn!("Unknown job status {other}, defaulting to pending");
            JobStatus::Pending
        }
    };

    let payload = serde_json::from_str(&payload).context("Failed to decode job payload")?;
    let result = match result {
        Some(value) => Some(serde_json::from_str(&value).context("Failed to decode job result")?),
        None => None,
    };

    let created_at = DateTime::parse_from_rfc3339(&created_at)
        .context("Failed to parse job created_at")?
        .with_timezone(&Utc);
    let updated_at = DateTime::parse_from_rfc3339(&updated_at)
        .context("Failed to parse job updated_at")?
        .with_timezone(&Utc);

    Ok(Job {
        id: Uuid::parse_str(&id).context("Invalid job id")?,
        status,
        payload,
        result,
        created_at,
        updated_at,
    })
}

async fn insert_audit(state: &AppState, record: &AuditRecord) -> Result<()> {
    let payload =
        serde_json::to_string(&record.payload).context("Failed to serialize audit payload")?;
    let created_at = record.created_at.to_rfc3339();

    match state.db_flavor {
        DbFlavor::Postgres => {
            sqlx::query(
                "INSERT INTO audits (id, event_type, actor, payload, created_at)
                 VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(record.id.to_string())
            .bind(&record.event_type)
            .bind(&record.actor)
            .bind(payload)
            .bind(created_at)
            .execute(&state.db)
            .await
            .context("Failed to insert audit record")?;
        }
        DbFlavor::Sqlite => {
            sqlx::query(
                "INSERT INTO audits (id, event_type, actor, payload, created_at)
                 VALUES (?, ?, ?, ?, ?)",
            )
            .bind(record.id.to_string())
            .bind(&record.event_type)
            .bind(&record.actor)
            .bind(payload)
            .bind(created_at)
            .execute(&state.db)
            .await
            .context("Failed to insert audit record")?;
        }
    }

    Ok(())
}

async fn fetch_audits(state: &AppState, query: AuditQuery) -> Result<Vec<AuditRecord>> {
    let limit = query.limit.unwrap_or(50).clamp(1, 500);

    let rows = match (state.db_flavor, query.event_type) {
        (DbFlavor::Postgres, Some(event_type)) => sqlx::query(
            "SELECT id, event_type, actor, payload, created_at FROM audits
                 WHERE event_type = $1
                 ORDER BY created_at DESC
                 LIMIT $2",
        )
        .bind(event_type)
        .bind(limit)
        .fetch_all(&state.db)
        .await
        .context("Failed to fetch audits")?,
        (DbFlavor::Postgres, None) => sqlx::query(
            "SELECT id, event_type, actor, payload, created_at FROM audits
                 ORDER BY created_at DESC
                 LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&state.db)
        .await
        .context("Failed to fetch audits")?,
        (DbFlavor::Sqlite, Some(event_type)) => sqlx::query(
            "SELECT id, event_type, actor, payload, created_at FROM audits
                 WHERE event_type = ?
                 ORDER BY created_at DESC
                 LIMIT ?",
        )
        .bind(event_type)
        .bind(limit)
        .fetch_all(&state.db)
        .await
        .context("Failed to fetch audits")?,
        (DbFlavor::Sqlite, None) => sqlx::query(
            "SELECT id, event_type, actor, payload, created_at FROM audits
                 ORDER BY created_at DESC
                 LIMIT ?",
        )
        .bind(limit)
        .fetch_all(&state.db)
        .await
        .context("Failed to fetch audits")?,
    };

    rows.into_iter().map(map_audit_row).collect()
}

fn map_audit_row(row: sqlx::any::AnyRow) -> Result<AuditRecord> {
    let id: String = row.get("id");
    let event_type: String = row.get("event_type");
    let actor: Option<String> = row.get("actor");
    let payload: String = row.get("payload");
    let created_at: String = row.get("created_at");

    let payload = serde_json::from_str(&payload).context("Failed to decode audit payload")?;
    let created_at = DateTime::parse_from_rfc3339(&created_at)
        .context("Failed to parse audit created_at")?
        .with_timezone(&Utc);

    Ok(AuditRecord {
        id: Uuid::parse_str(&id).context("Invalid audit id")?,
        event_type,
        actor,
        payload,
        created_at,
    })
}

fn internal_error(err: anyhow::Error) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}

fn db_flavor_from_url(database_url: &str) -> Result<DbFlavor> {
    if database_url.starts_with("postgres:") || database_url.starts_with("postgresql:") {
        Ok(DbFlavor::Postgres)
    } else if database_url.starts_with("sqlite:") {
        Ok(DbFlavor::Sqlite)
    } else {
        Err(anyhow::anyhow!(
            "Unsupported DATABASE_URL scheme (expected postgres or sqlite)"
        ))
    }
}
