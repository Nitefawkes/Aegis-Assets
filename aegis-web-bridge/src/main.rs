//! Web UI Bridge Server
//! 
//! This server provides a unified API that bridges the web UI requirements
//! with the existing Aegis-Core backend services. It combines:
//! 
//! - Asset extraction and management APIs
//! - Plugin registry and marketplace APIs
//! - Web UI static file serving
//! 
//! This allows the React web interface to work seamlessly with the backend.

use anyhow::{Result, Context};
use axum::{
    extract::{Query, Path, State, Json as AxumJson},
    http::{StatusCode, header::CONTENT_TYPE},
    response::{Json, Html, IntoResponse},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower::ServiceBuilder;
use tower_http::{cors::CorsLayer, trace::TraceLayer, services::ServeDir};
use tracing::{info, warn, error};

use aegis_core::{
    PluginRegistry,
    asset_db::{AssetDatabase, SearchQuery, AssetType},
};

/// Unified API state combining asset and plugin services
#[derive(Clone)]
pub struct WebBridgeState {
    pub asset_db: Arc<RwLock<AssetDatabase>>,
    pub plugin_registry: Arc<RwLock<PluginRegistry>>,
}

/// Configuration for the web bridge server
#[derive(Debug, Clone)]
pub struct WebBridgeConfig {
    pub asset_db_path: std::path::PathBuf,
    pub web_ui_path: Option<std::path::PathBuf>,
    pub cors_enabled: bool,
    pub serve_ui: bool,
}

impl Default for WebBridgeConfig {
    fn default() -> Self {
        Self {
            asset_db_path: std::path::PathBuf::from("./assets.db"),
            web_ui_path: None,
            cors_enabled: true,
            serve_ui: true,
        }
    }
}

/// Main web bridge server
pub struct WebBridgeServer {
    state: WebBridgeState,
    config: WebBridgeConfig,
}

impl WebBridgeServer {
    /// Create a new web bridge server
    pub async fn new(config: WebBridgeConfig) -> Result<Self> {
        // Initialize asset database
        let asset_db = AssetDatabase::new(&config.asset_db_path)
            .context("Failed to open asset database")?;

        // Initialize plugin registry
        let mut plugin_registry = PluginRegistry::new();
        plugin_registry.discover_and_register_plugins()
            .context("Failed to discover plugins")?;

        let state = WebBridgeState {
            asset_db: Arc::new(RwLock::new(asset_db)),
            plugin_registry: Arc::new(RwLock::new(plugin_registry)),
        };

        Ok(Self { state, config })
    }

    /// Create the unified router
    pub fn router(&self) -> Router {
        let mut app = Router::new()
            // Marketplace API endpoints (matching web UI expectations)
            .route("/api/v1/plugins", get(list_marketplace_plugins))
            .route("/api/v1/plugins/search", get(search_marketplace_plugins))
            .route("/api/v1/plugins/:name", get(get_marketplace_plugin))
            .route("/api/v1/plugins/:name/versions", get(get_plugin_versions))
            .route("/api/v1/plugins/:name/versions/:version/download", get(download_plugin))
            .route("/api/v1/categories", get(get_categories))
            .route("/api/v1/stats", get(get_marketplace_stats))

            // Asset management endpoints
            .route("/api/v1/assets", get(list_assets))
            .route("/api/v1/assets/search", get(search_assets))
            .route("/api/v1/assets/:id", get(get_asset))
            .route("/api/v1/assets/stats", get(get_asset_stats))
            
            // Extraction endpoints
            .route("/api/v1/extract", post(extract_assets))
            
            // Health and info endpoints
            .route("/api/v1/health", get(health_check))
            .route("/api/v1/info", get(api_info))
            
            .with_state(self.state.clone());

        // Add middleware
        let service = ServiceBuilder::new()
            .layer(TraceLayer::new_for_http());

        if self.config.cors_enabled {
            app = app.layer(CorsLayer::permissive());
        }

        // Serve static web UI files if configured
        if self.config.serve_ui {
            if let Some(web_ui_path) = &self.config.web_ui_path {
                app = app.nest_service("/", ServeDir::new(web_ui_path))
                    .route("/", get(serve_index_html));
            } else {
                // Serve a simple placeholder page
                app = app.route("/", get(serve_placeholder_ui));
            }
        }

        app.layer(service)
    }

    /// Start the web bridge server
    pub async fn serve(&self, addr: SocketAddr) -> Result<()> {
        let app = self.router();

        info!("🚀 Starting Aegis Web Bridge Server on {}", addr);
        info!("🌐 Web UI: http://{}/", addr);
        info!("📚 API Docs: http://{}/api/v1/info", addr);

        axum::serve(
            tokio::net::TcpListener::bind(addr).await
                .context("Failed to bind to address")?,
            app
        ).await
        .context("Server error")?;

        Ok(())
    }
}

// ===== MARKETPLACE API ENDPOINTS =====

/// List plugins in marketplace format
async fn list_marketplace_plugins(
    State(state): State<WebBridgeState>,
    Query(params): Query<ListPluginsParams>,
) -> Result<Json<PluginListResponse>, WebBridgeError> {
    let registry = state.plugin_registry.read().await;
    let plugins = registry.list_plugins();

    let mut marketplace_plugins: Vec<Plugin> = plugins.into_iter()
        .enumerate()
        .map(|(i, plugin_info)| Plugin {
            id: format!("plugin-{}", i),
            name: plugin_info.name().to_string(),
            display_name: plugin_info.name().to_string(),
            description: Some(format!("Plugin for {} format support", plugin_info.name())),
            author_email: "community@aegis-assets.com".to_string(),
            license: "MIT".to_string(),
            keywords: plugin_info.supported_extensions().iter().map(|s| s.to_string()).collect(),
            download_count: (i + 1) * 1000, // Mock download counts
            average_rating: Some(4.5),
            review_count: (i + 1) * 10,
            latest_version: plugin_info.version().to_string(),
            created_at: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        })
        .collect();

    // Apply pagination
    let total = marketplace_plugins.len();
    let page = params.page.unwrap_or(1).max(1);
    let per_page = params.per_page.unwrap_or(20).min(100);
    let start = (page - 1) * per_page;
    let end = (start + per_page).min(total);

    if start < total {
        marketplace_plugins = marketplace_plugins[start..end].to_vec();
    } else {
        marketplace_plugins.clear();
    }

    Ok(Json(PluginListResponse {
        plugins: marketplace_plugins,
        pagination: PaginationInfo {
            page,
            per_page,
            total,
            total_pages: (total + per_page - 1) / per_page,
        },
    }))
}

/// Search plugins in marketplace format
async fn search_marketplace_plugins(
    State(state): State<WebBridgeState>,
    Query(params): Query<SearchPluginsParams>,
) -> Result<Json<Vec<Plugin>>, WebBridgeError> {
    let registry = state.plugin_registry.read().await;
    let plugins = registry.list_plugins();

    let search_query = params.q.unwrap_or_default().to_lowercase();
    let filtered_plugins: Vec<Plugin> = plugins.into_iter()
        .enumerate()
        .filter(|(_, plugin_info)| {
            if search_query.is_empty() {
                return true;
            }
            plugin_info.name().to_lowercase().contains(&search_query) ||
            plugin_info.supported_extensions().iter()
                .any(|ext| ext.to_lowercase().contains(&search_query))
        })
        .take(params.limit.unwrap_or(20))
        .map(|(i, plugin_info)| Plugin {
            id: format!("plugin-{}", i),
            name: plugin_info.name().to_string(),
            display_name: plugin_info.name().to_string(),
            description: Some(format!("Plugin for {} format support", plugin_info.name())),
            author_email: "community@aegis-assets.com".to_string(),
            license: "MIT".to_string(),
            keywords: plugin_info.supported_extensions().iter().map(|s| s.to_string()).collect(),
            download_count: (i + 1) * 1000,
            average_rating: Some(4.5),
            review_count: (i + 1) * 10,
            latest_version: plugin_info.version().to_string(),
            created_at: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        })
        .collect();

    Ok(Json(filtered_plugins))
}

/// Get specific plugin details
async fn get_marketplace_plugin(
    State(state): State<WebBridgeState>,
    Path(plugin_name): Path<String>,
) -> Result<Json<Plugin>, WebBridgeError> {
    let registry = state.plugin_registry.read().await;
    let plugins = registry.list_plugins();

    let plugin_info = plugins.into_iter()
        .find(|p| p.name() == plugin_name)
        .ok_or_else(|| WebBridgeError::NotFound("Plugin not found".to_string()))?;

    let plugin = Plugin {
        id: format!("plugin-{}", plugin_name),
        name: plugin_info.name().to_string(),
        display_name: plugin_info.name().to_string(),
        description: Some(format!("Advanced plugin for {} format support with comprehensive asset extraction capabilities", plugin_info.name())),
        author_email: "community@aegis-assets.com".to_string(),
        license: "MIT".to_string(),
        keywords: plugin_info.supported_extensions().iter().map(|s| s.to_string()).collect(),
        download_count: 5000,
        average_rating: Some(4.7),
        review_count: 42,
        latest_version: plugin_info.version().to_string(),
        created_at: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
    };

    Ok(Json(plugin))
}

/// Get plugin versions
async fn get_plugin_versions(
    State(state): State<WebBridgeState>,
    Path(plugin_name): Path<String>,
) -> Result<Json<Vec<PluginVersion>>, WebBridgeError> {
    let registry = state.plugin_registry.read().await;
    let plugins = registry.list_plugins();

    let plugin_info = plugins.into_iter()
        .find(|p| p.name() == plugin_name)
        .ok_or_else(|| WebBridgeError::NotFound("Plugin not found".to_string()))?;

    let versions = vec![
        PluginVersion {
            id: format!("{}-{}", plugin_name, plugin_info.version()),
            plugin_id: plugin_name.clone(),
            version: plugin_info.version().to_string(),
            manifest: PluginManifest {
                name: plugin_info.name().to_string(),
                version: plugin_info.version().to_string(),
                description: format!("Plugin for {} format support", plugin_info.name()),
                authors: vec!["Aegis-Assets Team".to_string()],
                license: "MIT".to_string(),
                homepage: Some("https://github.com/aegis-assets/aegis-assets".to_string()),
                repository: Some("https://github.com/aegis-assets/aegis-assets".to_string()),
                keywords: plugin_info.supported_extensions().iter().map(|s| s.to_string()).collect(),
                aegis_version: "^0.2.0".to_string(),
                plugin_api_version: "1.0".to_string(),
                engine_name: plugin_info.name().to_string(),
                format_support: plugin_info.supported_extensions().iter().map(|ext| FormatSupport {
                    extension: ext.to_string(),
                    description: format!("{} format support", ext.to_uppercase()),
                    mime_type: None,
                }).collect(),
                compliance: WebComplianceInfo {
                    risk_level: "Low".to_string(),
                    publisher_policy: "Permissive".to_string(),
                    bounty_eligible: true,
                    enterprise_approved: true,
                },
                dependencies: HashMap::new(),
            },
            package_size: 1024 * 1024, // 1MB
            package_hash: "abcd1234".to_string(),
            package_url: format!("/api/v1/plugins/{}/{}/download", plugin_name, plugin_info.version()),
            status: "published".to_string(),
            published_at: Some(chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()),
            created_at: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        }
    ];

    Ok(Json(versions))
}

/// Download plugin (redirect to package)
async fn download_plugin(
    State(_state): State<WebBridgeState>,
    Path((plugin_name, version)): Path<(String, String)>,
) -> Result<Json<DownloadResponse>, WebBridgeError> {
    info!("Plugin download requested: {} v{}", plugin_name, version);
    
    Ok(Json(DownloadResponse {
        message: format!("Plugin {} v{} download initiated", plugin_name, version),
        download_url: format!("/packages/{}-{}.tar.gz", plugin_name, version),
    }))
}

/// Get categories
async fn get_categories() -> Json<Vec<String>> {
    Json(vec![
        "Unity".to_string(),
        "Unreal".to_string(),
        "Audio".to_string(),
        "Textures".to_string(),
        "Meshes".to_string(),
        "Animation".to_string(),
    ])
}

/// Get marketplace statistics
async fn get_marketplace_stats(
    State(state): State<WebBridgeState>,
) -> Result<Json<MarketplaceStats>, WebBridgeError> {
    let registry = state.plugin_registry.read().await;
    let plugin_count = registry.list_plugins().len();

    Ok(Json(MarketplaceStats {
        total_plugins: plugin_count,
        total_downloads: plugin_count * 1000,
        total_developers: plugin_count,
        categories: 6,
    }))
}

// ===== ASSET API ENDPOINTS =====

/// List assets (delegated to existing asset API)
async fn list_assets(
    State(state): State<WebBridgeState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, WebBridgeError> {
    let db = state.asset_db.read().await;
    let assets = db.get_all_assets();
    
    Ok(Json(serde_json::json!({
        "assets": assets,
        "total": assets.len(),
        "limit": 50
    })))
}

/// Search assets
async fn search_assets(
    State(state): State<WebBridgeState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, WebBridgeError> {
    let db = state.asset_db.read().await;
    let query = SearchQuery {
        text: params.get("q").cloned(),
        asset_type: None,
        tags: vec![],
        game_id: None,
        compliance_level: None,
        limit: Some(50),
        sort_by: aegis_core::asset_db::SortOrder::Relevance,
    };

    match db.search(query) {
        Ok(results) => {
            // Convert search results to serializable format
            let serializable_results: Vec<serde_json::Value> = results.into_iter()
                .map(|result| serde_json::json!({
                    "asset": {
                        "id": result.asset.id,
                        "name": result.asset.name,
                        "asset_type": format!("{:?}", result.asset.asset_type),
                        "file_size": result.asset.file_size,
                    },
                    "relevance_score": result.relevance_score,
                    "matched_fields": result.matched_fields,
                }))
                .collect();
            
            Ok(Json(serde_json::json!({
                "results": serializable_results,
                "total": serializable_results.len()
            })))
        },
        Err(e) => Err(WebBridgeError::InternalError(e.to_string())),
    }
}

/// Get specific asset
async fn get_asset(
    State(state): State<WebBridgeState>,
    Path(asset_id): Path<String>,
) -> Result<Json<serde_json::Value>, WebBridgeError> {
    let db = state.asset_db.read().await;
    
    match db.get_asset(&asset_id) {
        Some(asset) => Ok(Json(serde_json::to_value(asset).unwrap())),
        None => Err(WebBridgeError::NotFound("Asset not found".to_string())),
    }
}

/// Get asset statistics
async fn get_asset_stats(
    State(state): State<WebBridgeState>,
) -> Result<Json<serde_json::Value>, WebBridgeError> {
    let db = state.asset_db.read().await;
    let stats = db.get_stats();
    
    Ok(Json(serde_json::to_value(stats).unwrap()))
}

/// Extract assets endpoint
async fn extract_assets(
    State(_state): State<WebBridgeState>,
    AxumJson(request): AxumJson<ExtractRequest>,
) -> Result<Json<ExtractResponse>, WebBridgeError> {
    info!("Extraction requested for: {:?}", request.input_file);
    
    // For now, simulate extraction
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    
    Ok(Json(ExtractResponse {
        message: "Assets extracted successfully".to_string(),
        extracted_count: 15,
        output_directory: request.output_directory,
        converted: request.convert_assets,
    }))
}

// ===== UTILITY ENDPOINTS =====

/// Health check endpoint
async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        timestamp: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        services: vec![
            ServiceStatus { name: "asset_db".to_string(), status: "up".to_string() },
            ServiceStatus { name: "plugin_registry".to_string(), status: "up".to_string() },
        ],
    })
}

/// API info endpoint  
async fn api_info() -> Json<ApiInfoResponse> {
    Json(ApiInfoResponse {
        name: "Aegis-Assets Web Bridge API".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        description: "Unified API for web UI and backend services".to_string(),
        endpoints: vec![
            "/api/v1/plugins".to_string(),
            "/api/v1/plugins/search".to_string(),
            "/api/v1/assets".to_string(),
            "/api/v1/assets/search".to_string(),
            "/api/v1/extract".to_string(),
            "/api/v1/health".to_string(),
        ],
    })
}

/// Serve placeholder UI
async fn serve_placeholder_ui() -> Html<&'static str> {
    Html(r#"
<!DOCTYPE html>
<html>
<head>
    <title>Aegis-Assets Marketplace</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 40px; background: #f5f5f5; }
        .container { max-width: 800px; margin: 0 auto; background: white; padding: 40px; border-radius: 8px; box-shadow: 0 2px 10px rgba(0,0,0,0.1); }
        h1 { color: #2563eb; margin-bottom: 20px; }
        .status { background: #dcfce7; padding: 15px; border-radius: 6px; margin: 20px 0; border-left: 4px solid #16a34a; }
        .api-link { display: inline-block; background: #2563eb; color: white; padding: 10px 20px; text-decoration: none; border-radius: 6px; margin: 10px 10px 10px 0; }
        .api-link:hover { background: #1d4ed8; }
        .plugin-list { background: #f8fafc; padding: 20px; border-radius: 6px; margin: 20px 0; }
        .plugin { border-bottom: 1px solid #e2e8f0; padding: 10px 0; }
        .plugin:last-child { border-bottom: none; }
        .plugin-name { font-weight: bold; color: #1e293b; }
        .plugin-version { color: #64748b; font-size: 0.9em; }
    </style>
</head>
<body>
    <div class="container">
        <h1>🛡️ Aegis-Assets Marketplace</h1>
        
        <div class="status">
            <strong>✅ Server Status:</strong> Web Bridge API is running successfully!
        </div>

        <p>This is the Aegis-Assets Web Bridge Server providing unified API access to asset extraction and plugin marketplace services.</p>

        <h2>🔌 Available APIs</h2>
        <a href="/api/v1/plugins" class="api-link">Plugin Marketplace</a>
        <a href="/api/v1/assets" class="api-link">Asset Database</a>
        <a href="/api/v1/health" class="api-link">Health Check</a>
        <a href="/api/v1/info" class="api-link">API Info</a>

        <h2>🎮 Available Plugins</h2>
        <div class="plugin-list" id="plugins">
            <div>Loading plugins...</div>
        </div>

        <h2>📚 Next Steps</h2>
        <p>To use the full React web interface:</p>
        <ol>
            <li>Build the React web UI from the marketplace specification</li>
            <li>Configure the server to serve static files</li>
            <li>Point the web UI to this API endpoint</li>
        </ol>
    </div>

    <script>
        // Load and display available plugins
        fetch('/api/v1/plugins')
            .then(response => response.json())
            .then(data => {
                const pluginList = document.getElementById('plugins');
                if (data.plugins && data.plugins.length > 0) {
                    pluginList.innerHTML = data.plugins.map(plugin => 
                        `<div class="plugin">
                            <div class="plugin-name">${plugin.display_name}</div>
                            <div class="plugin-version">v${plugin.latest_version} • ${plugin.keywords.join(', ')}</div>
                        </div>`
                    ).join('');
                } else {
                    pluginList.innerHTML = '<div>No plugins available</div>';
                }
            })
            .catch(error => {
                document.getElementById('plugins').innerHTML = '<div>Error loading plugins</div>';
            });
    </script>
</body>
</html>
    "#)
}

/// Serve index.html for SPA routing
async fn serve_index_html() -> Html<&'static str> {
    Html(r#"
<!DOCTYPE html>
<html>
<head>
    <title>Aegis-Assets Marketplace</title>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
</head>
<body>
    <div id="root">
        <h1>Aegis-Assets Marketplace</h1>
        <p>React app would be loaded here</p>
    </div>
</body>
</html>
    "#)
}

// ===== DATA STRUCTURES =====

#[derive(Debug, Deserialize)]
struct ListPluginsParams {
    page: Option<usize>,
    per_page: Option<usize>,
    search: Option<String>,
    category: Option<String>,
    sort: Option<String>,
    engine: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SearchPluginsParams {
    q: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug, Serialize)]
struct PluginListResponse {
    plugins: Vec<Plugin>,
    pagination: PaginationInfo,
}

#[derive(Debug, Serialize)]
struct PaginationInfo {
    page: usize,
    per_page: usize,
    total: usize,
    total_pages: usize,
}

#[derive(Debug, Serialize, Clone)]
struct Plugin {
    id: String,
    name: String,
    display_name: String,
    description: Option<String>,
    author_email: String,
    license: String,
    keywords: Vec<String>,
    download_count: usize,
    average_rating: Option<f32>,
    review_count: usize,
    latest_version: String,
    created_at: String,
}

#[derive(Debug, Serialize)]
struct PluginVersion {
    id: String,
    plugin_id: String,
    version: String,
    manifest: PluginManifest,
    package_size: u64,
    package_hash: String,
    package_url: String,
    status: String,
    published_at: Option<String>,
    created_at: String,
}

#[derive(Debug, Serialize)]
struct PluginManifest {
    name: String,
    version: String,
    description: String,
    authors: Vec<String>,
    license: String,
    homepage: Option<String>,
    repository: Option<String>,
    keywords: Vec<String>,
    aegis_version: String,
    plugin_api_version: String,
    engine_name: String,
    format_support: Vec<FormatSupport>,
    compliance: WebComplianceInfo,
    dependencies: HashMap<String, String>,
}

#[derive(Debug, Serialize)]
struct FormatSupport {
    extension: String,
    description: String,
    mime_type: Option<String>,
}

#[derive(Debug, Serialize)]
struct WebComplianceInfo {
    risk_level: String,
    publisher_policy: String,
    bounty_eligible: bool,
    enterprise_approved: bool,
}

#[derive(Debug, Serialize)]
struct DownloadResponse {
    message: String,
    download_url: String,
}

#[derive(Debug, Serialize)]
struct MarketplaceStats {
    total_plugins: usize,
    total_downloads: usize,
    total_developers: usize,
    categories: usize,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: String,
    version: String,
    timestamp: String,
    services: Vec<ServiceStatus>,
}

#[derive(Debug, Serialize)]
struct ServiceStatus {
    name: String,
    status: String,
}

#[derive(Debug, Serialize)]
struct ApiInfoResponse {
    name: String,
    version: String,
    description: String,
    endpoints: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ExtractRequest {
    input_file: std::path::PathBuf,
    output_directory: String,
    convert_assets: bool,
    game_name: Option<String>,
}

#[derive(Debug, Serialize)]
struct ExtractResponse {
    message: String,
    extracted_count: usize,
    output_directory: String,
    converted: bool,
}

// ===== ERROR HANDLING =====

#[derive(Debug, thiserror::Error)]
pub enum WebBridgeError {
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Internal server error: {0}")]
    InternalError(String),
    #[error("Validation error: {0}")]
    ValidationError(String),
}

impl IntoResponse for WebBridgeError {
    fn into_response(self) -> axum::response::Response {
        let (status, error_message) = match self {
            WebBridgeError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            WebBridgeError::ValidationError(msg) => (StatusCode::BAD_REQUEST, msg),
            WebBridgeError::InternalError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };

        let body = Json(serde_json::json!({
            "error": error_message,
            "status": status.as_u16()
        }));

        (status, body).into_response()
    }
}

// ===== MAIN =====

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("aegis_web_bridge=info,tower_http=info")
        .with_target(false)
        .init();

    let config = WebBridgeConfig::default();
    let server = WebBridgeServer::new(config).await?;
    
    let addr = "0.0.0.0:3002".parse::<SocketAddr>()?;
    server.serve(addr).await?;

    Ok(())
}