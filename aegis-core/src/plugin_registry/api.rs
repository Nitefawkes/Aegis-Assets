//! REST API for plugin registry
//!
//! HTTP API endpoints for plugin discovery, management, and administration.

use axum::{
    extract::{Query, Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post, put, delete},
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tower::ServiceBuilder;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use anyhow::{Result, Context};
use tracing::{info, warn, error};

use super::{models::*, operations::PluginOperations, PluginRegistryConfig};

/// API state shared across all endpoints
#[derive(Clone)]
pub struct ApiState {
    pub registry: PluginRegistry,
}

/// API configuration
#[derive(Debug, Clone)]
pub struct ApiConfig {
    pub cors_enabled: bool,
    pub rate_limit: Option<u32>,
    pub admin_api_key: Option<String>,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            cors_enabled: true,
            rate_limit: Some(100), // 100 requests per minute
            admin_api_key: None,
        }
    }
}

/// Plugin registry API server
pub struct PluginRegistryApi {
    state: ApiState,
    config: ApiConfig,
}

impl PluginRegistryApi {
    /// Create a new plugin registry API server
    pub fn new(registry: PluginRegistry, config: ApiConfig) -> Self {
        Self {
            state: ApiState { registry },
            config,
        }
    }

    /// Create the router with all API endpoints
    pub fn router(&self) -> Router {
        let mut router = Router::new()
            // Public plugin endpoints
            .route("/api/v1/plugins", get(list_plugins))
            .route("/api/v1/plugins/search", get(search_plugins))
            .route("/api/v1/plugins/:name", get(get_plugin))
            .route("/api/v1/plugins/:name/versions", get(get_plugin_versions))
            .route("/api/v1/plugins/:name/:version/download", get(download_plugin))

            // Plugin statistics
            .route("/api/v1/plugins/:name/stats", get(get_plugin_stats))
            .route("/api/v1/plugins/:name/reviews", get(get_plugin_reviews))

            // Registry statistics
            .route("/api/v1/registry/stats", get(get_registry_stats))

            // Admin endpoints (protected)
            .route("/api/v1/admin/plugins", get(list_all_plugins_admin))
            .route("/api/v1/admin/plugins/:id/status", put(update_plugin_status))
            .route("/api/v1/admin/plugins/:name/:version", delete(delete_plugin_version))

            .with_state(self.state.clone());

        // Add middleware
        let service = ServiceBuilder::new()
            .layer(TraceLayer::new_for_http());

        if self.config.cors_enabled {
            router = router.layer(CorsLayer::permissive());
        }

        router.layer(service)
    }

    /// Start the API server
    pub async fn serve(&self, addr: std::net::SocketAddr) -> Result<()> {
        let app = self.router();

        info!("🚀 Starting Plugin Registry API server on {}", addr);
        info!("📚 API Documentation: http://{}/api/v1/registry/stats", addr);

        axum::serve(
            tokio::net::TcpListener::bind(addr).await
                .context("Failed to bind to address")?,
            app
        ).await
        .context("Server error")?;

        Ok(())
    }
}

// ===== API ENDPOINT HANDLERS =====

/// List approved plugins
async fn list_plugins(
    State(state): State<ApiState>,
    Query(params): Query<ListPluginsParams>,
) -> Result<Json<PluginListResponse>, ApiError> {
    let criteria = SearchCriteria {
        sort_by: SearchSort::Popularity,
        limit: params.limit,
        offset: params.offset,
        ..Default::default()
    };

    let plugins = state.registry.operations().search_plugins(criteria)
        .map_err(|e| ApiError::InternalError(e.to_string()))?;

    Ok(Json(PluginListResponse {
        plugins: plugins.into_iter().map(PluginSummary::from).collect(),
        total: plugins.len(),
        limit: params.limit.unwrap_or(50),
        offset: params.offset.unwrap_or(0),
    }))
}

/// Search plugins by criteria
async fn search_plugins(
    State(state): State<ApiState>,
    Query(params): Query<SearchPluginsParams>,
) -> Result<Json<PluginListResponse>, ApiError> {
    let mut criteria = SearchCriteria {
        query: params.q,
        engine: params.engine,
        category: params.category,
        risk_level: params.risk_level,
        sort_by: params.sort_by.unwrap_or(SearchSort::Relevance),
        limit: params.limit,
        offset: params.offset,
    };

    let plugins = state.registry.operations().search_plugins(criteria)
        .map_err(|e| ApiError::InternalError(e.to_string()))?;

    Ok(Json(PluginListResponse {
        plugins: plugins.into_iter().map(PluginSummary::from).collect(),
        total: plugins.len(),
        limit: params.limit.unwrap_or(50),
        offset: params.offset.unwrap_or(0),
    }))
}

/// Get plugin details by name
async fn get_plugin(
    State(state): State<ApiState>,
    Path(plugin_name): Path<String>,
) -> Result<Json<PluginDetailResponse>, ApiError> {
    let plugin = state.registry.operations().find_plugin(&plugin_name)
        .map_err(|e| ApiError::InternalError(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound(format!("Plugin not found: {}", plugin_name)))?;

    let versions = state.registry.operations().get_plugin_versions(&plugin_name)
        .map_err(|e| ApiError::InternalError(e.to_string()))?;

    let download_stats = state.registry.operations().get_download_stats(&plugin_name)
        .map_err(|e| ApiError::InternalError(e.to_string()))?;

    let (rating, reviews) = state.registry.operations().get_plugin_reviews(&plugin.id)
        .map_err(|e| ApiError::InternalError(e.to_string()))?;

    Ok(Json(PluginDetailResponse {
        plugin,
        versions,
        download_stats,
        rating: Some(rating),
        reviews,
    }))
}

/// Get plugin versions
async fn get_plugin_versions(
    State(state): State<ApiState>,
    Path(plugin_name): Path<String>,
) -> Result<Json<Vec<PluginVersion>>, ApiError> {
    let versions = state.registry.operations().get_plugin_versions(&plugin_name)
        .map_err(|e| ApiError::InternalError(e.to_string()))?;

    Ok(Json(versions))
}

/// Download plugin package
async fn download_plugin(
    State(state): State<ApiState>,
    Path((plugin_name, version)): Path<(String, String)>,
) -> Result<Json<PluginDownloadResponse>, ApiError> {
    // Record download
    state.registry.operations_mut().record_download(&plugin_name, &version, None)
        .map_err(|e| ApiError::InternalError(e.to_string()))?;

    // In a real implementation, this would return the actual plugin package
    // For now, return download metadata
    Ok(Json(PluginDownloadResponse {
        plugin_name,
        version,
        download_url: format!("/api/v1/plugins/{}/{}/package", plugin_name, version),
        message: "Download recorded successfully".to_string(),
    }))
}

/// Get plugin statistics
async fn get_plugin_stats(
    State(state): State<ApiState>,
    Path(plugin_name): Path<String>,
) -> Result<Json<PluginStatsResponse>, ApiError> {
    let stats = state.registry.operations().get_download_stats(&plugin_name)
        .map_err(|e| ApiError::InternalError(e.to_string()))?;

    Ok(Json(PluginStatsResponse { stats }))
}

/// Get plugin reviews
async fn get_plugin_reviews(
    State(state): State<ApiState>,
    Path(plugin_name): Path<String>,
) -> Result<Json<PluginReviewsResponse>, ApiError> {
    let plugin = state.registry.operations().find_plugin(&plugin_name)
        .map_err(|e| ApiError::InternalError(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound(format!("Plugin not found: {}", plugin_name)))?;

    let (rating, reviews) = state.registry.operations().get_plugin_reviews(&plugin.id)
        .map_err(|e| ApiError::InternalError(e.to_string()))?;

    Ok(Json(PluginReviewsResponse { rating, reviews }))
}

/// Get registry statistics
async fn get_registry_stats(
    State(state): State<ApiState>,
) -> Result<Json<RegistryStatsResponse>, ApiError> {
    let stats = state.registry.operations().stats()
        .map_err(|e| ApiError::InternalError(e.to_string()))?;

    Ok(Json(RegistryStatsResponse { stats }))
}

// ===== ADMIN ENDPOINTS =====

/// List all plugins (admin only)
async fn list_all_plugins_admin(
    State(state): State<ApiState>,
    Query(params): Query<AdminListParams>,
) -> Result<Json<AdminPluginListResponse>, ApiError> {
    // TODO: Add admin authentication

    let mut plugins = Vec::new();

    // Get approved plugins
    if params.include_approved.unwrap_or(true) {
        let approved = state.registry.operations().list_plugins()
            .map_err(|e| ApiError::InternalError(e.to_string()))?;
        plugins.extend(approved);
    }

    // Get pending plugins
    if params.include_pending.unwrap_or(false) {
        let pending = state.registry.operations().get_pending_plugins()
            .map_err(|e| ApiError::InternalError(e.to_string()))?;
        // Convert to PluginMetadata for response
        for version in pending {
            if let Ok(Some(plugin)) = state.registry.operations().get_plugin_by_id(&version.plugin_id) {
                plugins.push(plugin);
            }
        }
    }

    Ok(Json(AdminPluginListResponse {
        plugins,
        total: plugins.len(),
    }))
}

/// Update plugin status (admin only)
async fn update_plugin_status(
    State(state): State<ApiState>,
    Path(plugin_id): Path<String>,
    Json(request): Json<UpdateStatusRequest>,
) -> Result<Json<StatusUpdateResponse>, ApiError> {
    // TODO: Add admin authentication

    state.registry.operations_mut().update_plugin_status(&plugin_id, request.status)
        .map_err(|e| ApiError::InternalError(e.to_string()))?;

    Ok(Json(StatusUpdateResponse {
        plugin_id,
        status: request.status,
        message: "Plugin status updated successfully".to_string(),
    }))
}

/// Delete plugin version (admin only)
async fn delete_plugin_version(
    State(state): State<ApiState>,
    Path((plugin_name, version)): Path<(String, String)>,
) -> Result<Json<DeleteResponse>, ApiError> {
    // TODO: Add admin authentication

    state.registry.operations_mut().delete_plugin_version(&plugin_name, &version)
        .map_err(|e| ApiError::InternalError(e.to_string()))?;

    Ok(Json(DeleteResponse {
        message: format!("Plugin version {} {} deleted successfully", plugin_name, version),
    }))
}

// ===== API DATA STRUCTURES =====

/// Query parameters for listing plugins
#[derive(Debug, Deserialize)]
struct ListPluginsParams {
    limit: Option<usize>,
    offset: Option<usize>,
}

/// Query parameters for searching plugins
#[derive(Debug, Deserialize)]
struct SearchPluginsParams {
    q: Option<String>,
    engine: Option<String>,
    category: Option<String>,
    risk_level: Option<String>,
    sort_by: Option<SearchSort>,
    limit: Option<usize>,
    offset: Option<usize>,
}

/// Query parameters for admin listing
#[derive(Debug, Deserialize)]
struct AdminListParams {
    include_approved: Option<bool>,
    include_pending: Option<bool>,
    include_rejected: Option<bool>,
}

/// Response for plugin list
#[derive(Debug, Serialize)]
struct PluginListResponse {
    plugins: Vec<PluginSummary>,
    total: usize,
    limit: usize,
    offset: usize,
}

/// Response for plugin details
#[derive(Debug, Serialize)]
struct PluginDetailResponse {
    plugin: PluginMetadata,
    versions: Vec<PluginVersion>,
    download_stats: DownloadStats,
    rating: Option<PluginRating>,
    reviews: Vec<PluginReview>,
}

/// Response for plugin statistics
#[derive(Debug, Serialize)]
struct PluginStatsResponse {
    stats: DownloadStats,
}

/// Response for plugin reviews
#[derive(Debug, Serialize)]
struct PluginReviewsResponse {
    rating: PluginRating,
    reviews: Vec<PluginReview>,
}

/// Response for registry statistics
#[derive(Debug, Serialize)]
struct RegistryStatsResponse {
    stats: RegistryStats,
}

/// Response for admin plugin list
#[derive(Debug, Serialize)]
struct AdminPluginListResponse {
    plugins: Vec<PluginMetadata>,
    total: usize,
}

/// Request for status update
#[derive(Debug, Deserialize)]
struct UpdateStatusRequest {
    status: PluginStatus,
}

/// Response for status update
#[derive(Debug, Serialize)]
struct StatusUpdateResponse {
    plugin_id: String,
    status: PluginStatus,
    message: String,
}

/// Response for deletion
#[derive(Debug, Serialize)]
struct DeleteResponse {
    message: String,
}

/// Response for plugin download
#[derive(Debug, Serialize)]
struct PluginDownloadResponse {
    plugin_name: String,
    version: String,
    download_url: String,
    message: String,
}

// ===== HELPER TYPES =====

/// Plugin summary for listings
#[derive(Debug, Serialize)]
struct PluginSummary {
    name: String,
    display_name: String,
    description: Option<String>,
    version: String,
    author_email: String,
    license: String,
    keywords: Vec<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<PluginMetadata> for PluginSummary {
    fn from(metadata: PluginMetadata) -> Self {
        Self {
            name: metadata.name,
            display_name: metadata.display_name,
            description: metadata.description,
            version: metadata.version,
            author_email: metadata.author_email,
            license: metadata.license,
            keywords: metadata.keywords,
            created_at: metadata.created_at,
            updated_at: metadata.updated_at,
        }
    }
}

// ===== ERROR HANDLING =====

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Validation error: {0}")]
    ValidationError(String),
    #[error("Internal server error: {0}")]
    InternalError(String),
    #[error("Unauthorized: {0}")]
    Unauthorized(String),
    #[error("Forbidden: {0}")]
    Forbidden(String),
}

impl axum::response::IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, error_message) = match self {
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            ApiError::ValidationError(msg) => (StatusCode::BAD_REQUEST, msg),
            ApiError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg),
            ApiError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg),
            ApiError::InternalError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };

        let body = Json(serde_json::json!({
            "error": error_message,
            "status": status.as_u16()
        }));

        (status, body).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_summary_from_metadata() {
        let metadata = PluginMetadata {
            id: "test-id".to_string(),
            name: "test-plugin".to_string(),
            display_name: "Test Plugin".to_string(),
            description: Some("A test plugin".to_string()),
            author_email: "test@example.com".to_string(),
            license: "MIT".to_string(),
            homepage: None,
            repository: None,
            keywords: vec!["test".to_string()],
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            version: "1.0.0".to_string(),
            status: PluginStatus::Approved,
            package_size: 1024,
            package_hash: "test".to_string(),
            manifest: PluginManifest {
                package: PackageInfo {
                    name: "test-plugin".to_string(),
                    version: "1.0.0".to_string(),
                    description: Some("A test plugin".to_string()),
                    authors: vec!["test@example.com".to_string()],
                    license: "MIT".to_string(),
                    homepage: None,
                    repository: None,
                    keywords: vec!["test".to_string()],
                },
                plugin: PluginInfo {
                    aegis_version: "^0.2.0".to_string(),
                    plugin_api_version: "1.0".to_string(),
                    engine_name: Some("Unity".to_string()),
                    format_support: vec![],
                    features: vec![],
                },
                compliance: ComplianceInfo {
                    risk_level: RiskLevel::Low,
                    publisher_policy: PublisherPolicy::Permissive,
                    bounty_eligible: true,
                    enterprise_approved: true,
                    notes: None,
                },
                dependencies: HashMap::new(),
                dev_dependencies: HashMap::new(),
                build: None,
                testing: None,
                security: None,
            },
        };

        let summary = PluginSummary::from(metadata);
        assert_eq!(summary.name, "test-plugin");
        assert_eq!(summary.display_name, "Test Plugin");
        assert_eq!(summary.version, "1.0.0");
    }
}
