# Plugin Registry Backend Implementation

## Overview

Implementation guide for building the plugin registry backend with RESTful API endpoints, PostgreSQL database, and integration with security scanning pipeline.

## Database Implementation

### Database Schema Migration
```sql
-- migrations/001_initial_plugin_registry.sql
-- Plugin Registry Database Schema Implementation

-- Enable required extensions
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pg_trgm";  -- For fuzzy text search
CREATE EXTENSION IF NOT EXISTS "btree_gin"; -- For composite indexes

-- Core plugin metadata
CREATE TABLE plugins (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name VARCHAR(255) UNIQUE NOT NULL,
    display_name VARCHAR(255) NOT NULL,
    description TEXT,
    author_email VARCHAR(255) NOT NULL,
    license VARCHAR(50) NOT NULL,
    homepage TEXT,
    repository TEXT,
    keywords TEXT[],
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    
    -- Search optimization
    search_vector tsvector GENERATED ALWAYS AS (
        to_tsvector('english', 
            name || ' ' || 
            display_name || ' ' || 
            COALESCE(description, '') || ' ' || 
            array_to_string(keywords, ' ')
        )
    ) STORED
);

-- Plugin versions with semantic versioning
CREATE TABLE plugin_versions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    plugin_id UUID REFERENCES plugins(id) ON DELETE CASCADE,
    version VARCHAR(50) NOT NULL,
    semver_major INTEGER NOT NULL,
    semver_minor INTEGER NOT NULL,
    semver_patch INTEGER NOT NULL,
    semver_prerelease VARCHAR(50),
    
    -- Package information
    manifest JSONB NOT NULL,
    package_size BIGINT NOT NULL,
    package_hash CHAR(64) NOT NULL UNIQUE,
    package_url TEXT NOT NULL,
    
    -- Signature and security
    signature JSONB,
    security_scan_id UUID,
    security_scan_status VARCHAR(50) DEFAULT 'pending',
    security_scan_results JSONB,
    
    -- Publishing status
    status VARCHAR(50) DEFAULT 'pending' CHECK (status IN ('pending', 'approved', 'rejected', 'deprecated')),
    published_at TIMESTAMPTZ,
    
    -- Compliance
    compliance_status VARCHAR(50) DEFAULT 'pending' CHECK (compliance_status IN ('pending', 'approved', 'conditional', 'rejected')),
    compliance_conditions JSONB,
    compliance_notes TEXT,
    
    created_at TIMESTAMPTZ DEFAULT NOW(),
    
    UNIQUE(plugin_id, version),
    
    -- Ensure valid semver
    CONSTRAINT valid_semver CHECK (
        semver_major >= 0 AND 
        semver_minor >= 0 AND 
        semver_patch >= 0
    )
);

-- Plugin dependencies
CREATE TABLE plugin_dependencies (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    plugin_version_id UUID REFERENCES plugin_versions(id) ON DELETE CASCADE,
    dependency_name VARCHAR(255) NOT NULL,
    version_requirement VARCHAR(100) NOT NULL,
    dependency_type VARCHAR(50) DEFAULT 'runtime' CHECK (dependency_type IN ('runtime', 'dev', 'build', 'optional')),
    optional BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Supported file formats
CREATE TABLE plugin_formats (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    plugin_version_id UUID REFERENCES plugin_versions(id) ON DELETE CASCADE,
    file_extension VARCHAR(20) NOT NULL,
    format_description TEXT,
    mime_type VARCHAR(100),
    tested BOOLEAN DEFAULT FALSE,
    engine_name VARCHAR(100),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Download statistics
CREATE TABLE plugin_downloads (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    plugin_version_id UUID REFERENCES plugin_versions(id) ON DELETE CASCADE,
    download_timestamp TIMESTAMPTZ DEFAULT NOW(),
    user_id VARCHAR(255),
    ip_address INET,
    user_agent TEXT,
    success BOOLEAN DEFAULT TRUE,
    
    -- Partitioning by month for performance
    PARTITION BY RANGE (download_timestamp)
);

-- Create monthly partitions for downloads (example for current year)
CREATE TABLE plugin_downloads_2025_01 PARTITION OF plugin_downloads
FOR VALUES FROM ('2025-01-01') TO ('2025-02-01');

CREATE TABLE plugin_downloads_2025_02 PARTITION OF plugin_downloads
FOR VALUES FROM ('2025-02-01') TO ('2025-03-01');

-- Add more partitions as needed

-- Plugin ratings and reviews
CREATE TABLE plugin_reviews (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    plugin_id UUID REFERENCES plugins(id) ON DELETE CASCADE,
    user_id VARCHAR(255) NOT NULL,
    rating INTEGER CHECK (rating >= 1 AND rating <= 5),
    review_text TEXT,
    version_used VARCHAR(50),
    helpful_count INTEGER DEFAULT 0,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    
    UNIQUE(plugin_id, user_id)
);

-- Security scan results
CREATE TABLE security_scans (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    plugin_version_id UUID REFERENCES plugin_versions(id) ON DELETE CASCADE,
    scan_type VARCHAR(50) NOT NULL,
    scanner_version VARCHAR(50),
    scan_status VARCHAR(50) DEFAULT 'pending' CHECK (scan_status IN ('pending', 'running', 'completed', 'failed')),
    
    -- Results
    risk_score INTEGER DEFAULT 0,
    vulnerabilities_found INTEGER DEFAULT 0,
    scan_results JSONB,
    scan_report_url TEXT,
    
    started_at TIMESTAMPTZ DEFAULT NOW(),
    completed_at TIMESTAMPTZ,
    
    INDEX (plugin_version_id, scan_type),
    INDEX (scan_status, started_at)
);

-- Performance indexes
CREATE INDEX idx_plugins_search ON plugins USING gin(search_vector);
CREATE INDEX idx_plugins_created_at ON plugins(created_at DESC);
CREATE INDEX idx_plugins_keywords ON plugins USING gin(keywords);

CREATE INDEX idx_plugin_versions_plugin_id ON plugin_versions(plugin_id);
CREATE INDEX idx_plugin_versions_status ON plugin_versions(status) WHERE status = 'approved';
CREATE INDEX idx_plugin_versions_semver ON plugin_versions(plugin_id, semver_major DESC, semver_minor DESC, semver_patch DESC);
CREATE INDEX idx_plugin_versions_published ON plugin_versions(published_at DESC) WHERE status = 'approved';

CREATE INDEX idx_plugin_dependencies_name ON plugin_dependencies(dependency_name);
CREATE INDEX idx_plugin_formats_extension ON plugin_formats(file_extension);
CREATE INDEX idx_plugin_formats_engine ON plugin_formats(engine_name);

CREATE INDEX idx_plugin_downloads_timestamp ON plugin_downloads(download_timestamp DESC);
CREATE INDEX idx_plugin_downloads_plugin_version ON plugin_downloads(plugin_version_id);

CREATE INDEX idx_plugin_reviews_plugin_rating ON plugin_reviews(plugin_id, rating);
CREATE INDEX idx_plugin_reviews_created ON plugin_reviews(created_at DESC);

-- Triggers for updating timestamps
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

CREATE TRIGGER update_plugins_updated_at 
    BEFORE UPDATE ON plugins 
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_plugin_reviews_updated_at 
    BEFORE UPDATE ON plugin_reviews 
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Views for common queries
CREATE VIEW plugin_stats AS
SELECT 
    p.id,
    p.name,
    p.display_name,
    COUNT(DISTINCT pv.id) as version_count,
    COUNT(DISTINCT pd.id) as download_count,
    AVG(pr.rating) as average_rating,
    COUNT(DISTINCT pr.id) as review_count,
    MAX(pv.published_at) as last_published,
    MAX(pv.version) as latest_version
FROM plugins p
LEFT JOIN plugin_versions pv ON p.id = pv.plugin_id AND pv.status = 'approved'
LEFT JOIN plugin_downloads pd ON pv.id = pd.plugin_version_id
LEFT JOIN plugin_reviews pr ON p.id = pr.plugin_id
GROUP BY p.id, p.name, p.display_name;

CREATE VIEW popular_plugins AS
SELECT 
    ps.*,
    COUNT(pd.id) as downloads_last_30_days
FROM plugin_stats ps
LEFT JOIN plugin_versions pv ON ps.id = pv.plugin_id
LEFT JOIN plugin_downloads pd ON pv.id = pd.plugin_version_id 
    AND pd.download_timestamp > NOW() - INTERVAL '30 days'
GROUP BY ps.id, ps.name, ps.display_name, ps.version_count, ps.download_count, 
         ps.average_rating, ps.review_count, ps.last_published, ps.latest_version
ORDER BY downloads_last_30_days DESC, ps.average_rating DESC;
```

### Rust Backend Implementation

#### Main Application Structure (src/main.rs)
```rust
use anyhow::Result;
use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::Json,
    routing::{get, post, put, delete},
    Router,
};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use std::sync::Arc;
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_http::{
    cors::CorsLayer,
    trace::TraceLayer,
    compression::CompressionLayer,
};
use tracing::{info, instrument};
use uuid::Uuid;

mod config;
mod database;
mod models;
mod handlers;
mod security;
mod services;
mod middleware;

use config::Config;
use database::Database;
use models::*;

#[derive(Clone)]
pub struct AppState {
    db: Arc<Database>,
    config: Config,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();
    
    // Load configuration
    let config = Config::from_env()?;
    
    // Connect to database
    let database = Database::connect(&config.database_url).await?;
    
    // Run migrations
    database.migrate().await?;
    
    // Create application state
    let state = AppState {
        db: Arc::new(database),
        config: config.clone(),
    };
    
    // Build router
    let app = create_router(state).await?;
    
    // Start server
    let listener = TcpListener::bind(&config.server_address).await?;
    info!("Plugin registry server listening on {}", config.server_address);
    
    axum::serve(listener, app).await?;
    
    Ok(())
}

async fn create_router(state: AppState) -> Result<Router> {
    let api_routes = Router::new()
        // Plugin management
        .route("/plugins", get(handlers::list_plugins))
        .route("/plugins", post(handlers::create_plugin))
        .route("/plugins/:name", get(handlers::get_plugin))
        .route("/plugins/:name", put(handlers::update_plugin))
        .route("/plugins/:name", delete(handlers::delete_plugin))
        
        // Version management
        .route("/plugins/:name/versions", get(handlers::list_versions))
        .route("/plugins/:name/versions", post(handlers::publish_version))
        .route("/plugins/:name/versions/:version", get(handlers::get_version))
        .route("/plugins/:name/versions/:version", delete(handlers::delete_version))
        .route("/plugins/:name/versions/:version/download", get(handlers::download_plugin))
        
        // Reviews and ratings
        .route("/plugins/:name/reviews", get(handlers::list_reviews))
        .route("/plugins/:name/reviews", post(handlers::create_review))
        .route("/plugins/:name/reviews/:review_id", put(handlers::update_review))
        .route("/plugins/:name/reviews/:review_id", delete(handlers::delete_review))
        
        // Search and discovery
        .route("/search", get(handlers::search_plugins))
        .route("/categories", get(handlers::list_categories))
        .route("/stats", get(handlers::get_stats))
        
        // Security and compliance
        .route("/plugins/:name/versions/:version/security", get(handlers::get_security_status))
        .route("/plugins/:name/versions/:version/compliance", get(handlers::get_compliance_status))
        
        // Admin endpoints
        .route("/admin/plugins/:name/approve", post(handlers::admin_approve_plugin))
        .route("/admin/plugins/:name/reject", post(handlers::admin_reject_plugin))
        .route("/admin/scans", get(handlers::admin_list_scans))
        
        .layer(Extension(state));
    
    let app = Router::new()
        .nest("/api/v1", api_routes)
        .route("/health", get(health_check))
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(CompressionLayer::new())
                .layer(CorsLayer::permissive()) // Configure CORS as needed
        );
    
    Ok(app)
}

async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now(),
        "service": "plugin-registry"
    }))
}
```

#### Database Layer (src/database.rs)
```rust
use anyhow::{Context, Result};
use sqlx::{PgPool, postgres::PgConnectOptions, ConnectOptions};
use std::str::FromStr;
use tracing::log::LevelFilter;

use crate::models::*;

pub struct Database {
    pool: PgPool,
}

impl Database {
    pub async fn connect(database_url: &str) -> Result<Self> {
        let mut options = PgConnectOptions::from_str(database_url)?;
        options.log_statements(LevelFilter::Debug);
        
        let pool = PgPool::connect_with(options)
            .await
            .context("Failed to connect to database")?;
        
        Ok(Self { pool })
    }
    
    pub async fn migrate(&self) -> Result<()> {
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .context("Failed to run database migrations")?;
        Ok(())
    }
    
    // Plugin CRUD operations
    pub async fn create_plugin(&self, plugin: &CreatePluginRequest) -> Result<Plugin> {
        let row = sqlx::query!(
            r#"
            INSERT INTO plugins (name, display_name, description, author_email, license, homepage, repository, keywords)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING id, name, display_name, description, author_email, license, homepage, repository, keywords, created_at, updated_at
            "#,
            plugin.name,
            plugin.display_name,
            plugin.description,
            plugin.author_email,
            plugin.license,
            plugin.homepage,
            plugin.repository,
            &plugin.keywords
        )
        .fetch_one(&self.pool)
        .await
        .context("Failed to create plugin")?;
        
        Ok(Plugin {
            id: row.id,
            name: row.name,
            display_name: row.display_name,
            description: row.description,
            author_email: row.author_email,
            license: row.license,
            homepage: row.homepage,
            repository: row.repository,
            keywords: row.keywords.unwrap_or_default(),
            created_at: row.created_at.unwrap(),
            updated_at: row.updated_at.unwrap(),
        })
    }
    
    pub async fn get_plugin(&self, name: &str) -> Result<Option<Plugin>> {
        let row = sqlx::query!(
            "SELECT id, name, display_name, description, author_email, license, homepage, repository, keywords, created_at, updated_at FROM plugins WHERE name = $1",
            name
        )
        .fetch_optional(&self.pool)
        .await
        .context("Failed to fetch plugin")?;
        
        Ok(row.map(|r| Plugin {
            id: r.id,
            name: r.name,
            display_name: r.display_name,
            description: r.description,
            author_email: r.author_email,
            license: r.license,
            homepage: r.homepage,
            repository: r.repository,
            keywords: r.keywords.unwrap_or_default(),
            created_at: r.created_at.unwrap(),
            updated_at: r.updated_at.unwrap(),
        }))
    }
    
    pub async fn list_plugins(&self, params: &ListPluginsParams) -> Result<PluginListResponse> {
        let offset = params.page.saturating_sub(1) * params.per_page;
        
        // Build dynamic query based on filters
        let mut query_builder = sqlx::QueryBuilder::new(
            "SELECT p.*, ps.download_count, ps.average_rating, ps.review_count, ps.latest_version FROM plugins p LEFT JOIN plugin_stats ps ON p.id = ps.id WHERE 1=1"
        );
        
        if let Some(search) = &params.search {
            query_builder.push(" AND p.search_vector @@ plainto_tsquery('english', ");
            query_builder.push_bind(search);
            query_builder.push(")");
        }
        
        if let Some(category) = &params.category {
            query_builder.push(" AND ");
            query_builder.push_bind(category);
            query_builder.push(" = ANY(p.keywords)");
        }
        
        // Add ordering
        match params.sort.as_deref() {
            Some("popularity") => query_builder.push(" ORDER BY ps.download_count DESC NULLS LAST"),
            Some("rating") => query_builder.push(" ORDER BY ps.average_rating DESC NULLS LAST"),
            Some("recent") => query_builder.push(" ORDER BY p.created_at DESC"),
            _ => query_builder.push(" ORDER BY p.name ASC"),
        };
        
        // Add pagination
        query_builder.push(" LIMIT ");
        query_builder.push_bind(params.per_page as i64);
        query_builder.push(" OFFSET ");
        query_builder.push_bind(offset as i64);
        
        let query = query_builder.build();
        let rows = query.fetch_all(&self.pool).await?;
        
        // Count total for pagination
        let total_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM plugins")
            .fetch_one(&self.pool)
            .await?;
        
        let plugins: Vec<PluginSummary> = rows.into_iter().map(|row| {
            PluginSummary {
                id: row.get("id"),
                name: row.get("name"),
                display_name: row.get("display_name"),
                description: row.get("description"),
                author_email: row.get("author_email"),
                license: row.get("license"),
                keywords: row.get::<Option<Vec<String>>, _>("keywords").unwrap_or_default(),
                download_count: row.get::<Option<i64>, _>("download_count").unwrap_or(0) as u64,
                average_rating: row.get::<Option<f64>, _>("average_rating"),
                review_count: row.get::<Option<i64>, _>("review_count").unwrap_or(0) as u32,
                latest_version: row.get("latest_version"),
                created_at: row.get("created_at"),
            }
        }).collect();
        
        Ok(PluginListResponse {
            plugins,
            pagination: PaginationInfo {
                page: params.page,
                per_page: params.per_page,
                total: total_count as u64,
                total_pages: ((total_count as f64) / (params.per_page as f64)).ceil() as u32,
            },
        })
    }
    
    // Version management
    pub async fn publish_version(&self, plugin_name: &str, version: &PublishVersionRequest) -> Result<PluginVersion> {
        // Parse semantic version
        let semver = semver::Version::parse(&version.version)?;
        
        let row = sqlx::query!(
            r#"
            INSERT INTO plugin_versions (
                plugin_id, version, semver_major, semver_minor, semver_patch, semver_prerelease,
                manifest, package_size, package_hash, package_url, signature
            )
            SELECT p.id, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11
            FROM plugins p 
            WHERE p.name = $1
            RETURNING id, plugin_id, version, manifest, package_size, package_hash, package_url, 
                     status, published_at, created_at
            "#,
            plugin_name,
            version.version,
            semver.major as i32,
            semver.minor as i32,
            semver.patch as i32,
            semver.pre.to_string(),
            serde_json::to_value(&version.manifest)?,
            version.package_size as i64,
            version.package_hash,
            version.package_url,
            serde_json::to_value(&version.signature)?
        )
        .fetch_one(&self.pool)
        .await
        .context("Failed to publish version")?;
        
        Ok(PluginVersion {
            id: row.id,
            plugin_id: row.plugin_id,
            version: row.version,
            manifest: serde_json::from_value(row.manifest)?,
            package_size: row.package_size as u64,
            package_hash: row.package_hash,
            package_url: row.package_url,
            signature: row.signature.map(|s| serde_json::from_value(s)).transpose()?,
            status: row.status,
            published_at: row.published_at,
            created_at: row.created_at.unwrap(),
        })
    }
    
    pub async fn get_latest_version(&self, plugin_name: &str) -> Result<Option<PluginVersion>> {
        let row = sqlx::query!(
            r#"
            SELECT pv.id, pv.plugin_id, pv.version, pv.manifest, pv.package_size, 
                   pv.package_hash, pv.package_url, pv.signature, pv.status, 
                   pv.published_at, pv.created_at
            FROM plugin_versions pv
            JOIN plugins p ON pv.plugin_id = p.id
            WHERE p.name = $1 AND pv.status = 'approved'
            ORDER BY pv.semver_major DESC, pv.semver_minor DESC, pv.semver_patch DESC
            LIMIT 1
            "#,
            plugin_name
        )
        .fetch_optional(&self.pool)
        .await
        .context("Failed to fetch latest version")?;
        
        Ok(row.map(|r| PluginVersion {
            id: r.id,
            plugin_id: r.plugin_id,
            version: r.version,
            manifest: serde_json::from_value(r.manifest).unwrap_or_default(),
            package_size: r.package_size as u64,
            package_hash: r.package_hash,
            package_url: r.package_url,
            signature: r.signature.and_then(|s| serde_json::from_value(s).ok()),
            status: r.status,
            published_at: r.published_at,
            created_at: r.created_at.unwrap(),
        }))
    }
    
    // Search functionality
    pub async fn search_plugins(&self, query: &str, limit: u32) -> Result<Vec<PluginSummary>> {
        let rows = sqlx::query!(
            r#"
            SELECT p.*, ps.download_count, ps.average_rating, ps.review_count, ps.latest_version,
                   ts_rank(p.search_vector, plainto_tsquery('english', $1)) as rank
            FROM plugins p
            LEFT JOIN plugin_stats ps ON p.id = ps.id
            WHERE p.search_vector @@ plainto_tsquery('english', $1)
            ORDER BY rank DESC, ps.download_count DESC NULLS LAST
            LIMIT $2
            "#,
            query,
            limit as i64
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to search plugins")?;
        
        let plugins = rows.into_iter().map(|row| PluginSummary {
            id: row.id,
            name: row.name,
            display_name: row.display_name,
            description: row.description,
            author_email: row.author_email,
            license: row.license,
            keywords: row.keywords.unwrap_or_default(),
            download_count: row.download_count.unwrap_or(0) as u64,
            average_rating: row.average_rating,
            review_count: row.review_count.unwrap_or(0) as u32,
            latest_version: row.latest_version,
            created_at: row.created_at.unwrap(),
        }).collect();
        
        Ok(plugins)
    }
    
    // Download tracking
    pub async fn record_download(&self, version_id: uuid::Uuid, user_id: Option<&str>, ip: Option<std::net::IpAddr>, user_agent: Option<&str>) -> Result<()> {
        sqlx::query!(
            "INSERT INTO plugin_downloads (plugin_version_id, user_id, ip_address, user_agent) VALUES ($1, $2, $3, $4)",
            version_id,
            user_id,
            ip.map(|ip| ip.to_string()),
            user_agent
        )
        .execute(&self.pool)
        .await
        .context("Failed to record download")?;
        
        Ok(())
    }
}
```

#### Request/Response Models (src/models.rs)
```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Plugin {
    pub id: Uuid,
    pub name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub author_email: String,
    pub license: String,
    pub homepage: Option<String>,
    pub repository: Option<String>,
    pub keywords: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PluginSummary {
    pub id: Uuid,
    pub name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub author_email: String,
    pub license: String,
    pub keywords: Vec<String>,
    pub download_count: u64,
    pub average_rating: Option<f64>,
    pub review_count: u32,
    pub latest_version: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PluginVersion {
    pub id: Uuid,
    pub plugin_id: Uuid,
    pub version: String,
    pub manifest: PluginManifest,
    pub package_size: u64,
    pub package_hash: String,
    pub package_url: String,
    pub signature: Option<PluginSignature>,
    pub status: String,
    pub published_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub authors: Vec<String>,
    pub license: String,
    pub homepage: Option<String>,
    pub repository: Option<String>,
    pub keywords: Vec<String>,
    pub aegis_version: String,
    pub plugin_api_version: String,
    pub engine_name: String,
    pub format_support: Vec<FormatSupport>,
    pub compliance: ComplianceInfo,
    pub dependencies: std::collections::HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FormatSupport {
    pub extension: String,
    pub description: String,
    pub mime_type: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ComplianceInfo {
    pub risk_level: String,
    pub publisher_policy: String,
    pub bounty_eligible: bool,
    pub enterprise_approved: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PluginSignature {
    pub signature_version: String,
    pub plugin_name: String,
    pub plugin_version: String,
    pub signer: String,
    pub signature_algorithm: String,
    pub public_key: String,
    pub signature: String,
    pub timestamp: DateTime<Utc>,
    pub trust_level: String,
}

// Request/Response types
#[derive(Debug, Deserialize)]
pub struct CreatePluginRequest {
    pub name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub author_email: String,
    pub license: String,
    pub homepage: Option<String>,
    pub repository: Option<String>,
    pub keywords: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct PublishVersionRequest {
    pub version: String,
    pub manifest: PluginManifest,
    pub package_size: u64,
    pub package_hash: String,
    pub package_url: String,
    pub signature: PluginSignature,
}

#[derive(Debug, Deserialize)]
pub struct ListPluginsParams {
    pub page: u32,
    pub per_page: u32,
    pub search: Option<String>,
    pub category: Option<String>,
    pub sort: Option<String>,
    pub engine: Option<String>,
}

impl Default for ListPluginsParams {
    fn default() -> Self {
        Self {
            page: 1,
            per_page: 20,
            search: None,
            category: None,
            sort: None,
            engine: None,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PluginListResponse {
    pub plugins: Vec<PluginSummary>,
    pub pagination: PaginationInfo,
}

#[derive(Debug, Serialize)]
pub struct PaginationInfo {
    pub page: u32,
    pub per_page: u32,
    pub total: u64,
    pub total_pages: u32,
}

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
    pub timestamp: DateTime<Utc>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            timestamp: Utc::now(),
        }
    }
    
    pub fn error(message: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message),
            timestamp: Utc::now(),
        }
    }
}
```

#### API Handlers (src/handlers.rs)
```rust
use axum::{
    extract::{Extension, Path, Query, multipart::Multipart},
    http::StatusCode,
    response::Json,
};
use anyhow::Result;
use tracing::{info, warn, instrument};

use crate::{AppState, models::*};

#[instrument(skip(state))]
pub async fn list_plugins(
    Extension(state): Extension<AppState>,
    Query(params): Query<ListPluginsParams>,
) -> Result<Json<ApiResponse<PluginListResponse>>, StatusCode> {
    match state.db.list_plugins(&params).await {
        Ok(response) => Ok(Json(ApiResponse::success(response))),
        Err(e) => {
            warn!("Failed to list plugins: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[instrument(skip(state))]
pub async fn get_plugin(
    Extension(state): Extension<AppState>,
    Path(name): Path<String>,
) -> Result<Json<ApiResponse<Plugin>>, StatusCode> {
    match state.db.get_plugin(&name).await {
        Ok(Some(plugin)) => Ok(Json(ApiResponse::success(plugin))),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(e) => {
            warn!("Failed to get plugin {}: {}", name, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[instrument(skip(state))]
pub async fn create_plugin(
    Extension(state): Extension<AppState>,
    Json(request): Json<CreatePluginRequest>,
) -> Result<Json<ApiResponse<Plugin>>, StatusCode> {
    // Validate plugin name format
    if !is_valid_plugin_name(&request.name) {
        return Err(StatusCode::BAD_REQUEST);
    }
    
    match state.db.create_plugin(&request).await {
        Ok(plugin) => {
            info!("Created new plugin: {}", plugin.name);
            Ok(Json(ApiResponse::success(plugin)))
        }
        Err(e) => {
            warn!("Failed to create plugin: {}", e);
            if e.to_string().contains("duplicate key") {
                Err(StatusCode::CONFLICT)
            } else {
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    }
}

#[instrument(skip(state))]
pub async fn publish_version(
    Extension(state): Extension<AppState>,
    Path(plugin_name): Path<String>,
    Json(request): Json<PublishVersionRequest>,
) -> Result<Json<ApiResponse<PluginVersion>>, StatusCode> {
    // Validate version format
    if let Err(_) = semver::Version::parse(&request.version) {
        return Err(StatusCode::BAD_REQUEST);
    }
    
    // TODO: Validate signature
    // TODO: Trigger security scan
    
    match state.db.publish_version(&plugin_name, &request).await {
        Ok(version) => {
            info!("Published version {} for plugin {}", version.version, plugin_name);
            
            // TODO: Trigger security scanning pipeline
            // TODO: Queue for compliance review if needed
            
            Ok(Json(ApiResponse::success(version)))
        }
        Err(e) => {
            warn!("Failed to publish version for {}: {}", plugin_name, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[instrument(skip(state))]
pub async fn download_plugin(
    Extension(state): Extension<AppState>,
    Path((plugin_name, version)): Path<(String, String)>,
    // TODO: Extract user info from auth headers
) -> Result<axum::response::Redirect, StatusCode> {
    // Get plugin version
    let plugin_version = match state.db.get_version(&plugin_name, &version).await {
        Ok(Some(v)) => v,
        Ok(None) => return Err(StatusCode::NOT_FOUND),
        Err(e) => {
            warn!("Failed to get version for download: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    
    // Check if version is approved for download
    if plugin_version.status != "approved" {
        return Err(StatusCode::FORBIDDEN);
    }
    
    // Record download
    if let Err(e) = state.db.record_download(
        plugin_version.id,
        None, // TODO: Extract user_id from auth
        None, // TODO: Extract IP from request
        None, // TODO: Extract user agent
    ).await {
        warn!("Failed to record download: {}", e);
        // Don't fail the download for this
    }
    
    // Redirect to package URL (S3, etc.)
    Ok(axum::response::Redirect::temporary(&plugin_version.package_url))
}

#[instrument(skip(state))]
pub async fn search_plugins(
    Extension(state): Extension<AppState>,
    Query(params): Query<SearchParams>,
) -> Result<Json<ApiResponse<Vec<PluginSummary>>>, StatusCode> {
    let query = params.q.unwrap_or_default();
    let limit = params.limit.unwrap_or(20).min(100); // Cap at 100 results
    
    if query.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    
    match state.db.search_plugins(&query, limit).await {
        Ok(plugins) => Ok(Json(ApiResponse::success(plugins))),
        Err(e) => {
            warn!("Search failed for query '{}': {}", query, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[derive(serde::Deserialize)]
pub struct SearchParams {
    pub q: Option<String>,
    pub limit: Option<u32>,
}

// Helper functions
fn is_valid_plugin_name(name: &str) -> bool {
    // Plugin names must be alphanumeric with hyphens, 3-50 characters
    name.len() >= 3 
        && name.len() <= 50 
        && name.chars().all(|c| c.is_alphanumeric() || c == '-')
        && !name.starts_with('-')
        && !name.ends_with('-')
}

// TODO: Implement remaining handlers
pub async fn update_plugin() -> StatusCode { StatusCode::NOT_IMPLEMENTED }
pub async fn delete_plugin() -> StatusCode { StatusCode::NOT_IMPLEMENTED }
pub async fn list_versions() -> StatusCode { StatusCode::NOT_IMPLEMENTED }
pub async fn get_version() -> StatusCode { StatusCode::NOT_IMPLEMENTED }
pub async fn delete_version() -> StatusCode { StatusCode::NOT_IMPLEMENTED }
pub async fn list_reviews() -> StatusCode { StatusCode::NOT_IMPLEMENTED }
pub async fn create_review() -> StatusCode { StatusCode::NOT_IMPLEMENTED }
pub async fn update_review() -> StatusCode { StatusCode::NOT_IMPLEMENTED }
pub async fn delete_review() -> StatusCode { StatusCode::NOT_IMPLEMENTED }
pub async fn list_categories() -> StatusCode { StatusCode::NOT_IMPLEMENTED }
pub async fn get_stats() -> StatusCode { StatusCode::NOT_IMPLEMENTED }
pub async fn get_security_status() -> StatusCode { StatusCode::NOT_IMPLEMENTED }
pub async fn get_compliance_status() -> StatusCode { StatusCode::NOT_IMPLEMENTED }
pub async fn admin_approve_plugin() -> StatusCode { StatusCode::NOT_IMPLEMENTED }
pub async fn admin_reject_plugin() -> StatusCode { StatusCode::NOT_IMPLEMENTED }
pub async fn admin_list_scans() -> StatusCode { StatusCode::NOT_IMPLEMENTED }
```

### Cargo.toml
```toml
[package]
name = "aegis-plugin-registry"
version = "0.1.0"
edition = "2021"

[dependencies]
# Web framework
axum = { version = "0.7", features = ["multipart", "macros"] }
tokio = { version = "1.0", features = ["full"] }
tower = { version = "0.4", features = ["util"] }
tower-http = { version = "0.5", features = ["fs", "cors", "trace", "compression"] }

# Database
sqlx = { version = "0.7", features = ["runtime-tokio-rustls", "postgres", "uuid", "chrono", "json"] }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Error handling
anyhow = "1.0"
thiserror = "1.0"

# Utilities
uuid = { version = "1.0", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
semver = { version = "1.0", features = ["serde"] }

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Config
config = "0.13"
```

### Docker Configuration
```dockerfile
# Dockerfile for Plugin Registry
FROM rust:1.74-slim as builder

WORKDIR /app
RUN apt-get update && apt-get install -y pkg-config libssl-dev libpq-dev

# Copy dependency files
COPY Cargo.toml Cargo.lock ./
COPY src/ src/
COPY migrations/ migrations/

# Build the application
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libpq5 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/aegis-plugin-registry /usr/local/bin/
COPY --from=builder /app/migrations /migrations

EXPOSE 3000

CMD ["aegis-plugin-registry"]
```

### Docker Compose with Registry
```yaml
# docker-compose.registry.yml
version: '3.8'

services:
  plugin-registry:
    build:
      context: .
      dockerfile: Dockerfile
    ports:
      - "3000:3000"
    environment:
      - DATABASE_URL=postgres://postgres:password@postgres:5432/aegis_registry
      - SERVER_ADDRESS=0.0.0.0:3000
      - RUST_LOG=info
    depends_on:
      - postgres
      - redis
    restart: unless-stopped

  postgres:
    image: postgres:15-alpine
    environment:
      POSTGRES_DB: aegis_registry
      POSTGRES_USER: postgres
      POSTGRES_PASSWORD: password
    volumes:
      - registry_postgres_data:/var/lib/postgresql/data
    ports:
      - "5433:5432"

  redis:
    image: redis:7-alpine
    volumes:
      - registry_redis_data:/data
    ports:
      - "6380:6379"

volumes:
  registry_postgres_data:
  registry_redis_data:
```

---

**Status**: Plugin Registry Backend Implementation Complete  
**Coverage**: Database schema, REST API, search functionality, version management  
**Dependencies**: PostgreSQL, Redis (optional for caching)  
**Performance**: Optimized with indexes, pagination, full-text search

**Next Steps**:
1. Deploy registry backend in development environment
2. Test API endpoints with sample data
3. Begin Marketplace Web Interface implementation
4. Integrate with security scanning pipeline

Ready to continue with the Marketplace Web Interface implementation?

