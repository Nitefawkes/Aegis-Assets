//! REST API server for Aegis-Assets
//!
//! This module provides a REST API interface for accessing and managing
//! game assets through HTTP endpoints. The API enables:
//!
//! - Asset search and discovery
//! - Metadata browsing and filtering
//! - Asset extraction and conversion
//! - Database management operations
//!
//! # Example
//!
//! ```rust,no_run
//! use aegis_core::api::ApiServer;
//! use std::net::SocketAddr;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let server = ApiServer::new("./assets.db").await?;
//!     let addr = "0.0.0.0:3000".parse::<SocketAddr>()?;
//!     
//!     println!("🚀 Starting Aegis-Assets API server on {}", addr);
//!     server.serve(addr).await?;
//!     Ok(())
//! }
//! ```

#[cfg(feature = "api")]
pub mod server {
    use crate::asset_db::{AssetDatabase, SearchQuery, AssetType, SortOrder};
    use axum::{
        extract::{Query, State, Path},
        http::StatusCode,
        response::Json,
        routing::{get, post},
        Router,
    };
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;
    use std::net::SocketAddr;
    use std::path::PathBuf;
    use std::sync::Arc;
    use tokio::sync::RwLock;
    use tower::ServiceBuilder;
    use tower_http::{cors::CorsLayer, trace::TraceLayer};
    use anyhow::{Result, Context};

    /// API server state
    #[derive(Clone)]
    pub struct ApiState {
        pub db: Arc<RwLock<AssetDatabase>>,
    }

    /// API server configuration
    #[derive(Debug, Clone)]
    pub struct ApiConfig {
        pub db_path: PathBuf,
        pub cors_enabled: bool,
        pub rate_limit: Option<u32>,
    }

    impl Default for ApiConfig {
        fn default() -> Self {
            Self {
                db_path: PathBuf::from("./assets.db"),
                cors_enabled: true,
                rate_limit: Some(100), // 100 requests per minute
            }
        }
    }

    /// Main API server
    pub struct ApiServer {
        state: ApiState,
        config: ApiConfig,
    }

    impl ApiServer {
        /// Create a new API server
        pub async fn new(db_path: impl Into<PathBuf>) -> Result<Self> {
            let config = ApiConfig {
                db_path: db_path.into(),
                ..Default::default()
            };

            let db = AssetDatabase::new(&config.db_path)
                .context("Failed to open asset database")?;

            let state = ApiState {
                db: Arc::new(RwLock::new(db)),
            };

            Ok(Self { state, config })
        }

        /// Create a new API server with custom configuration
        pub async fn with_config(config: ApiConfig) -> Result<Self> {
            let db = AssetDatabase::new(&config.db_path)
                .context("Failed to open asset database")?;

            let state = ApiState {
                db: Arc::new(RwLock::new(db)),
            };

            Ok(Self { state, config })
        }

        /// Create the router with all API endpoints
        pub fn router(&self) -> Router {
            let mut router = Router::new()
                // Asset endpoints
                .route("/api/v1/assets", get(list_assets))
                .route("/api/v1/assets/search", get(search_assets))
                .route("/api/v1/assets/:id", get(get_asset))
                .route("/api/v1/assets/stats", get(get_stats))
                
                // Database management endpoints
                .route("/api/v1/db/index", post(index_assets))
                .route("/api/v1/db/stats", get(get_db_stats))
                
                // Health check
                .route("/api/v1/health", get(health_check))
                
                // API info
                .route("/api/v1/info", get(api_info))
                
                .with_state(self.state.clone());

            // Add middleware
            let service = ServiceBuilder::new()
                .layer(TraceLayer::new_for_http());

            if self.config.cors_enabled {
                router = router.layer(CorsLayer::permissive());
            }

            router.layer(service)
        }

        /// Start the server
        pub async fn serve(&self, addr: SocketAddr) -> Result<()> {
            let app = self.router();
            
            let listener = tokio::net::TcpListener::bind(addr).await
                .context("Failed to bind to address")?;

            tracing::info!("🚀 Aegis-Assets API server listening on {}", addr);
            
            axum::serve(listener, app).await
                .context("Server error")?;

            Ok(())
        }
    }

    // ===== API ENDPOINT HANDLERS =====

    /// Health check endpoint
    async fn health_check() -> Json<HealthResponse> {
        Json(HealthResponse {
            status: "healthy".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            timestamp: chrono::Utc::now(),
        })
    }

    /// API info endpoint
    async fn api_info() -> Json<ApiInfoResponse> {
        Json(ApiInfoResponse {
            name: "Aegis-Assets API".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            description: "REST API for game asset extraction and management".to_string(),
            endpoints: vec![
                "/api/v1/health".to_string(),
                "/api/v1/info".to_string(),
                "/api/v1/assets".to_string(),
                "/api/v1/assets/search".to_string(),
                "/api/v1/assets/{id}".to_string(),
                "/api/v1/assets/stats".to_string(),
                "/api/v1/db/index".to_string(),
                "/api/v1/db/stats".to_string(),
            ],
        })
    }

    /// List assets with optional filtering
    async fn list_assets(
        State(state): State<ApiState>,
        Query(params): Query<ListAssetsParams>,
    ) -> Result<Json<AssetsResponse>, ApiError> {
        let db = state.db.read().await;
        
        let assets = if let Some(asset_type) = params.asset_type {
            db.get_assets_by_type(&asset_type)
        } else {
            db.get_all_assets()
        };

        let limit = params.limit.unwrap_or(50).min(1000); // Max 1000 results
        let total = assets.len();
        let assets_to_return = assets.into_iter()
            .take(limit)
            .map(|asset| AssetResponse::from(asset.clone()))
            .collect();

        Ok(Json(AssetsResponse {
            assets: assets_to_return,
            total,
            limit,
        }))
    }

    /// Search assets
    async fn search_assets(
        State(state): State<ApiState>,
        Query(params): Query<SearchAssetsParams>,
    ) -> Result<Json<SearchResponse>, ApiError> {
        let db = state.db.read().await;

        let query_text = params.q.clone();
        let query = SearchQuery {
            text: params.q,
            asset_type: params.asset_type,
            tags: params.tags.unwrap_or_default(),
            game_id: params.game,
            compliance_level: params.compliance,
            limit: Some(params.limit.unwrap_or(50).min(1000)),
            sort_by: params.sort_by.unwrap_or(SortOrder::Relevance),
        };

        let results = db.search(query)
            .map_err(|e| ApiError::DatabaseError(e.to_string()))?;

        let search_results: Vec<SearchResultResponse> = results.into_iter()
            .map(|result| SearchResultResponse {
                asset: AssetResponse::from(result.asset),
                relevance_score: result.relevance_score,
                matched_fields: result.matched_fields,
            })
            .collect();

        let total = search_results.len();

        Ok(Json(SearchResponse {
            results: search_results,
            query: query_text.unwrap_or_default(),
            total,
        }))
    }

    /// Get a specific asset by ID
    async fn get_asset(
        State(state): State<ApiState>,
        Path(asset_id): Path<String>,
    ) -> Result<Json<AssetResponse>, ApiError> {
        let db = state.db.read().await;
        
        let asset = db.get_asset(&asset_id)
            .ok_or_else(|| ApiError::NotFound("Asset not found".to_string()))?;

        Ok(Json(AssetResponse::from(asset.clone())))
    }

    /// Get asset type statistics
    async fn get_stats(
        State(state): State<ApiState>,
    ) -> Result<Json<StatsResponse>, ApiError> {
        let db = state.db.read().await;
        let stats = db.get_stats();

        Ok(Json(StatsResponse {
            total_assets: stats.total_assets,
            total_size: stats.total_size,
            assets_by_type: stats.assets_by_type,
            tags: stats.tags,
        }))
    }

    /// Get database statistics (alias for stats)
    async fn get_db_stats(
        State(state): State<ApiState>,
    ) -> Result<Json<StatsResponse>, ApiError> {
        get_stats(State(state)).await
    }

    /// Index assets from a directory
    async fn index_assets(
        State(state): State<ApiState>,
        Json(request): Json<IndexRequest>,
    ) -> Result<Json<IndexResponse>, ApiError> {
        let mut db = state.db.write().await;
        let mut indexed_count = 0;

        // Walk through the directory and index assets
        for entry in walkdir::WalkDir::new(&request.directory)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file()) {

            let path = entry.path();
            let file_name = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");

            // Skip certain files
            if file_name.starts_with('.') || file_name == "assets.db" {
                continue;
            }

            // Determine asset type from file extension
            let asset_type = match path.extension().and_then(|e| e.to_str()) {
                Some("png") | Some("jpg") | Some("jpeg") | Some("tga") => AssetType::Texture,
                Some("gltf") | Some("glb") | Some("obj") | Some("fbx") => AssetType::Mesh,
                Some("wav") | Some("ogg") | Some("mp3") => AssetType::Audio,
                Some("anim") => AssetType::Animation,
                Some("mat") => AssetType::Material,
                Some("level") | Some("scene") => AssetType::Level,
                Some("cs") | Some("lua") => AssetType::Script,
                _ => AssetType::Other("Unknown".to_string()),
            };

            // Get file metadata
            let metadata = std::fs::metadata(path)
                .map_err(|e| ApiError::FileSystemError(e.to_string()))?;
            let file_size = metadata.len();

            // Generate content hash
            let content_hash = if file_size < 1024 * 1024 { // Only hash small files
                let data = std::fs::read(path)
                    .map_err(|e| ApiError::FileSystemError(e.to_string()))?;
                blake3::hash(&data).to_hex().to_string()
            } else {
                // For large files, use file path as hash
                blake3::hash(path.to_string_lossy().as_bytes()).to_hex().to_string()
            };

            // Create asset metadata
            let asset_metadata = crate::asset_db::AssetMetadata::new(
                format!("asset_{}", indexed_count),
                file_name.to_string(),
                asset_type,
                path.to_path_buf(),
                path.parent().unwrap_or(path).to_path_buf(),
                file_size,
                content_hash,
            )
            .with_game_id(request.game.clone().unwrap_or_else(|| "unknown".to_string()));

            // Add tags
            let mut asset_metadata = asset_metadata;
            for tag in &request.tags {
                asset_metadata = asset_metadata.with_tag(tag.clone());
            }

            db.index_asset(asset_metadata)
                .map_err(|e| ApiError::DatabaseError(e.to_string()))?;
            indexed_count += 1;
        }

        Ok(Json(IndexResponse {
            message: format!("Successfully indexed {} assets", indexed_count),
            indexed_count,
            directory: request.directory,
        }))
    }

    // ===== API DATA STRUCTURES =====

    #[derive(Debug, Serialize)]
    struct HealthResponse {
        status: String,
        version: String,
        #[serde(with = "chrono::serde::ts_seconds")]
        timestamp: chrono::DateTime<chrono::Utc>,
    }

    #[derive(Debug, Serialize)]
    struct ApiInfoResponse {
        name: String,
        version: String,
        description: String,
        endpoints: Vec<String>,
    }

    #[derive(Debug, Deserialize)]
    struct ListAssetsParams {
        asset_type: Option<AssetType>,
        limit: Option<usize>,
    }

    #[derive(Debug, Deserialize)]
    struct SearchAssetsParams {
        q: Option<String>,
        asset_type: Option<AssetType>,
        tags: Option<Vec<String>>,
        game: Option<String>,
        compliance: Option<String>,
        limit: Option<usize>,
        sort_by: Option<SortOrder>,
    }

    #[derive(Debug, Serialize)]
    struct AssetsResponse {
        assets: Vec<AssetResponse>,
        total: usize,
        limit: usize,
    }

    #[derive(Debug, Serialize)]
    struct SearchResponse {
        results: Vec<SearchResultResponse>,
        query: String,
        total: usize,
    }

    #[derive(Debug, Serialize)]
    struct SearchResultResponse {
        asset: AssetResponse,
        relevance_score: f32,
        matched_fields: Vec<String>,
    }

    #[derive(Debug, Serialize)]
    struct AssetResponse {
        id: String,
        name: String,
        asset_type: AssetType,
        source_path: PathBuf,
        output_path: PathBuf,
        file_size: u64,
        export_format: Option<crate::export::ExportFormat>,
        tags: Vec<String>,
        description: Option<String>,
        #[serde(with = "chrono::serde::ts_seconds")]
        created_at: chrono::DateTime<chrono::Utc>,
        #[serde(with = "chrono::serde::ts_seconds")]
        updated_at: chrono::DateTime<chrono::Utc>,
        game_id: Option<String>,
        compliance_level: String,
        content_hash: String,
    }

    impl From<crate::asset_db::AssetMetadata> for AssetResponse {
        fn from(metadata: crate::asset_db::AssetMetadata) -> Self {
            Self {
                id: metadata.id,
                name: metadata.name,
                asset_type: metadata.asset_type,
                source_path: metadata.source_path,
                output_path: metadata.output_path,
                file_size: metadata.file_size,
                export_format: metadata.export_format,
                tags: metadata.tags,
                description: metadata.description,
                created_at: metadata.created_at,
                updated_at: metadata.updated_at,
                game_id: metadata.game_id,
                compliance_level: metadata.compliance_level,
                content_hash: metadata.content_hash,
            }
        }
    }

    #[derive(Debug, Serialize)]
    struct StatsResponse {
        total_assets: usize,
        total_size: u64,
        assets_by_type: HashMap<String, usize>,
        tags: HashMap<String, usize>,
    }

    #[derive(Debug, Deserialize)]
    struct IndexRequest {
        directory: PathBuf,
        game: Option<String>,
        tags: Vec<String>,
    }

    #[derive(Debug, Serialize)]
    struct IndexResponse {
        message: String,
        indexed_count: usize,
        directory: PathBuf,
    }

    // ===== ERROR HANDLING =====

    #[derive(Debug, thiserror::Error)]
    pub enum ApiError {
        #[error("Not found: {0}")]
        NotFound(String),
        #[error("Database error: {0}")]
        DatabaseError(String),
        #[error("File system error: {0}")]
        FileSystemError(String),
        #[error("Validation error: {0}")]
        ValidationError(String),
        #[error("Internal server error: {0}")]
        InternalError(String),
    }

    impl axum::response::IntoResponse for ApiError {
        fn into_response(self) -> axum::response::Response {
            let (status, error_message) = match self {
                ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
                ApiError::ValidationError(msg) => (StatusCode::BAD_REQUEST, msg),
                ApiError::DatabaseError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
                ApiError::FileSystemError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
                ApiError::InternalError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            };

            let body = Json(serde_json::json!({
                "error": error_message,
                "status": status.as_u16()
            }));

            (status, body).into_response()
        }
    }
}

#[cfg(not(feature = "api"))]
pub mod server {
    //! Stub module when API feature is not enabled
    
    use anyhow::{Result, bail};
    use std::net::SocketAddr;
    use std::path::PathBuf;

    pub struct ApiServer;
    pub struct ApiConfig;

    impl ApiServer {
        pub async fn new(_db_path: impl Into<PathBuf>) -> Result<Self> {
            bail!("API feature is not enabled. Enable with --features api")
        }

        pub async fn serve(&self, _addr: SocketAddr) -> Result<()> {
            bail!("API feature is not enabled. Enable with --features api")
        }
    }
}

// Re-export for convenience
#[cfg(feature = "api")]
pub use server::*;

#[cfg(not(feature = "api"))]
pub use server::{ApiServer, ApiConfig};
