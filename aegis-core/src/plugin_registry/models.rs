//! Data models for the plugin registry system
//!
//! These structs represent the core data structures used throughout the plugin registry.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};

/// Plugin metadata representing a plugin in the registry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    /// Unique plugin identifier
    pub id: String,

    /// Plugin name (used as identifier)
    pub name: String,

    /// Display name for the plugin
    pub display_name: String,

    /// Plugin description
    pub description: Option<String>,

    /// Author's email address
    pub author_email: String,

    /// License under which the plugin is distributed
    pub license: String,

    /// Project homepage URL
    pub homepage: Option<String>,

    /// Repository URL
    pub repository: Option<String>,

    /// Keywords for search and categorization
    pub keywords: Vec<String>,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last update timestamp
    pub updated_at: DateTime<Utc>,

    /// Latest version information
    pub version: String,

    /// Current status of the plugin
    pub status: PluginStatus,

    /// Package size in bytes
    pub package_size: u64,

    /// Package hash for integrity verification
    pub package_hash: String,

    /// Plugin manifest data
    pub manifest: PluginManifest,
}

/// Plugin version information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginVersion {
    /// Unique version identifier
    pub id: String,

    /// Parent plugin ID
    pub plugin_id: String,

    /// Version string (semantic versioning)
    pub version: String,

    /// Version status
    pub status: PluginStatus,

    /// Package size in bytes
    pub package_size: u64,

    /// Package hash for integrity verification
    pub package_hash: String,

    /// Plugin manifest
    pub manifest: PluginManifest,

    /// Code signature data (if signed)
    pub signature: Option<CodeSignature>,

    /// Publication timestamp
    pub published_at: DateTime<Utc>,
}

/// Plugin manifest (parsed from plugin.toml)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    /// Package information
    pub package: PackageInfo,

    /// Plugin-specific configuration
    pub plugin: PluginInfo,

    /// Compliance information
    pub compliance: ComplianceInfo,

    /// Dependencies
    pub dependencies: HashMap<String, String>,

    /// Development dependencies
    pub dev_dependencies: HashMap<String, String>,

    /// Build configuration
    pub build: Option<BuildConfig>,

    /// Testing configuration
    pub testing: Option<TestingConfig>,

    /// Security configuration
    pub security: Option<SecurityConfig>,
}

/// Package information from plugin.toml [package] section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageInfo {
    /// Package name
    pub name: String,

    /// Package version
    pub version: String,

    /// Package description
    pub description: Option<String>,

    /// Authors
    pub authors: Vec<String>,

    /// License
    pub license: String,

    /// Homepage URL
    pub homepage: Option<String>,

    /// Repository URL
    pub repository: Option<String>,

    /// Keywords for categorization
    pub keywords: Vec<String>,
}

/// Plugin-specific configuration from plugin.toml [plugin] section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    /// Compatible Aegis version requirement
    pub aegis_version: String,

    /// Plugin API version requirement
    pub plugin_api_version: String,

    /// Target game engine
    pub engine_name: Option<String>,

    /// Supported file formats
    pub format_support: Vec<FileFormatInfo>,

    /// Plugin features
    pub features: Vec<String>,
}

/// File format information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileFormatInfo {
    /// File extension (without dot)
    pub extension: String,

    /// Description of the format
    pub description: String,

    /// MIME type if applicable
    pub mime_type: Option<String>,

    /// Whether this format has been tested
    pub tested: bool,
}

/// Compliance information from plugin.toml [compliance] section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceInfo {
    /// Risk level for this plugin
    pub risk_level: RiskLevel,

    /// Publisher's known stance on asset extraction
    pub publisher_policy: PublisherPolicy,

    /// Whether this plugin is eligible for bounties
    pub bounty_eligible: bool,

    /// Whether this plugin is approved for enterprise use
    pub enterprise_approved: bool,

    /// Additional compliance notes
    pub notes: Option<String>,
}

/// Risk levels for plugins
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RiskLevel {
    /// Low risk - generally safe to use
    Low,
    /// Medium risk - use with caution
    Medium,
    /// High risk - restricted or problematic
    High,
}

/// Publisher policies regarding asset extraction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PublisherPolicy {
    /// Publisher is generally permissive
    Permissive,
    /// Publisher has mixed signals
    Mixed,
    /// Publisher is actively hostile to extraction
    Aggressive,
    /// Publisher policy is unknown
    Unknown,
}

/// Build configuration from plugin.toml [build] section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildConfig {
    /// Minimum Rust version required
    pub min_rust_version: Option<String>,

    /// Features to enable
    pub features: Vec<String>,

    /// Target platforms
    pub targets: Vec<String>,
}

/// Testing configuration from plugin.toml [testing] section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestingConfig {
    /// Sample files for testing
    pub sample_files: Vec<String>,

    /// Required test success rate
    pub required_success_rate: f64,

    /// Performance benchmarks
    pub performance_benchmark: Option<PerformanceBenchmark>,
}

/// Performance benchmark configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceBenchmark {
    /// Maximum extraction time
    pub max_extraction_time: String,

    /// Maximum memory usage
    pub max_memory_usage: String,
}

/// Security configuration from plugin.toml [security] section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Required system permissions
    pub sandbox_permissions: Vec<SandboxPermission>,

    /// Code signing certificate level
    pub code_signing_cert: CodeSigningLevel,

    /// Vulnerability scan requirement
    pub vulnerability_scan: ScanRequirement,
}

/// Sandbox permissions for plugin execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SandboxPermission {
    /// Read access to input files
    FileRead,
    /// Write access to temporary directories
    FileWriteTemp,
    /// No subprocess execution
    ProcessSpawnNone,
    /// No network access
    NetworkNone,
    /// Limited filesystem access
    FilesystemRestricted,
}

/// Code signing certificate levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CodeSigningLevel {
    /// Community-signed
    Community,
    /// Verified developer
    Verified,
    /// Enterprise approved
    Enterprise,
}

/// Vulnerability scan requirements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScanRequirement {
    /// Scanning is required
    Required,
    /// Scanning is optional
    Optional,
    /// Scanning is disabled
    Disabled,
}

/// Code signature information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeSignature {
    /// Signature version
    pub signature_version: String,

    /// Plugin name
    pub plugin_name: String,

    /// Plugin version
    pub plugin_version: String,

    /// Signer information
    pub signer: String,

    /// Signature algorithm used
    pub signature_algorithm: String,

    /// Public key (base64 encoded)
    pub public_key: String,

    /// Signature data (base64 encoded)
    pub signature: String,

    /// Signature timestamp
    pub timestamp: DateTime<Utc>,

    /// Certificate chain
    pub cert_chain: Vec<String>,

    /// Trust level
    pub trust_level: String,
}

/// Plugin status enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginStatus {
    /// Plugin is pending review by administrators
    PendingReview,
    /// Plugin has been approved and is available
    Approved,
    /// Plugin has been rejected
    Rejected,
    /// Plugin has been deprecated
    Deprecated,
    /// Plugin has been removed
    Removed,
}

/// Registry statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryStats {
    /// Total number of plugins
    pub total_plugins: usize,
    /// Total number of plugin versions
    pub total_versions: usize,
    /// Total number of downloads
    pub total_downloads: usize,
    /// Number of plugins pending review
    pub pending_reviews: usize,
    /// Last updated timestamp
    pub last_updated: DateTime<Utc>,
}

/// Download statistics for a plugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadStats {
    /// Total downloads across all versions
    pub total_downloads: usize,
    /// Downloads in the last 30 days
    pub downloads_last_30_days: usize,
    /// Download count by version
    pub version_breakdown: HashMap<String, usize>,
}

/// Search criteria for plugin discovery
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchCriteria {
    /// Search query string
    pub query: Option<String>,
    /// Filter by game engine
    pub engine: Option<String>,
    /// Filter by category
    pub category: Option<String>,
    /// Filter by risk level
    pub risk_level: Option<String>,
    /// Sort order
    pub sort_by: SearchSort,
    /// Maximum number of results
    pub limit: Option<usize>,
    /// Result offset for pagination
    pub offset: Option<usize>,
}

/// Search sort options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SearchSort {
    /// Sort by relevance to search query
    Relevance,
    /// Sort by popularity (download count)
    Popularity,
    /// Sort by most recently updated
    RecentlyUpdated,
    /// Sort alphabetically by name
    Alphabetical,
}

impl Default for SearchSort {
    fn default() -> Self {
        Self::Relevance
    }
}

/// Plugin rating information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginRating {
    /// Average rating (1-5)
    pub average: f64,
    /// Number of ratings
    pub count: usize,
    /// Rating distribution
    pub distribution: HashMap<u8, usize>, // 1-5 star counts
}

/// Plugin review information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginReview {
    /// Review ID
    pub id: String,
    /// User ID who wrote the review
    pub user_id: String,
    /// Rating (1-5)
    pub rating: u8,
    /// Review text
    pub review_text: Option<String>,
    /// Version of plugin being reviewed
    pub version_used: Option<String>,
    /// Number of users who found this review helpful
    pub helpful_count: usize,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
}

/// Security scan result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityScanResult {
    /// Scan ID
    pub id: String,
    /// Plugin version ID
    pub plugin_version_id: String,
    /// Scan timestamp
    pub scan_timestamp: DateTime<Utc>,
    /// Scan status
    pub scan_status: ScanStatus,
    /// Number of vulnerabilities found
    pub vulnerability_count: usize,
    /// Scanner version used
    pub scanner_version: String,
    /// Detailed scan report
    pub scan_report: serde_json::Value,
}

/// Scan status enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScanStatus {
    /// Scan completed successfully
    Passed,
    /// Scan found issues
    Failed,
    /// Scan is in progress
    Pending,
    /// Scan was skipped
    Skipped,
}

/// Audit log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    /// Entry ID
    pub id: String,
    /// Table name affected
    pub table_name: String,
    /// Record ID affected
    pub record_id: String,
    /// Operation performed
    pub operation: AuditOperation,
    /// Old values (for updates/deletes)
    pub old_values: Option<serde_json::Value>,
    /// New values (for inserts/updates)
    pub new_values: Option<serde_json::Value>,
    /// Who made the change
    pub changed_by: Option<String>,
    /// When the change occurred
    pub change_timestamp: DateTime<Utc>,
    /// IP address of the change
    pub ip_address: Option<String>,
    /// User agent of the change
    pub user_agent: Option<String>,
}

/// Audit operation enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditOperation {
    /// Record was inserted
    Insert,
    /// Record was updated
    Update,
    /// Record was deleted
    Delete,
    /// Security scan was performed
    Scan,
    /// Plugin was downloaded
    Download,
}

/// Pagination information for search results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationInfo {
    /// Total number of results
    pub total: usize,
    /// Current page number (1-based)
    pub page: usize,
    /// Results per page
    pub per_page: usize,
    /// Total number of pages
    pub total_pages: usize,
    /// Whether there are more results
    pub has_more: bool,
}

/// Plugin search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginSearchResult {
    /// Plugin metadata
    pub plugin: PluginMetadata,
    /// Download statistics
    pub download_stats: DownloadStats,
    /// Rating information
    pub rating: Option<PluginRating>,
    /// Latest version
    pub latest_version: String,
    /// Last updated timestamp
    pub last_updated: DateTime<Utc>,
    /// Relevance score (0.0-1.0)
    pub relevance_score: f64,
}

/// Plugin marketplace information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMarketplaceInfo {
    /// Plugin metadata
    pub plugin: PluginMetadata,
    /// Download statistics
    pub download_stats: DownloadStats,
    /// Rating information
    pub rating: Option<PluginRating>,
    /// Reviews
    pub reviews: Vec<PluginReview>,
    /// Available versions
    pub versions: Vec<String>,
    /// Latest version
    pub latest_version: String,
    /// Last updated timestamp
    pub last_updated: DateTime<Utc>,
    /// Security scan status
    pub security_status: ScanStatus,
    /// Compliance information
    pub compliance_info: ComplianceInfo,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_metadata_serialization() {
        let metadata = PluginMetadata {
            id: "test-plugin-id".to_string(),
            name: "test-plugin".to_string(),
            display_name: "Test Plugin".to_string(),
            description: Some("A test plugin".to_string()),
            author_email: "test@example.com".to_string(),
            license: "MIT".to_string(),
            homepage: Some("https://example.com".to_string()),
            repository: Some("https://github.com/example/test-plugin".to_string()),
            keywords: vec!["test".to_string(), "plugin".to_string()],
            created_at: Utc::now(),
            updated_at: Utc::now(),
            version: "1.0.0".to_string(),
            status: PluginStatus::Approved,
            package_size: 1024,
            package_hash: "test-hash".to_string(),
            manifest: PluginManifest {
                package: PackageInfo {
                    name: "test-plugin".to_string(),
                    version: "1.0.0".to_string(),
                    description: Some("A test plugin".to_string()),
                    authors: vec!["test@example.com".to_string()],
                    license: "MIT".to_string(),
                    homepage: Some("https://example.com".to_string()),
                    repository: Some("https://github.com/example/test-plugin".to_string()),
                    keywords: vec!["test".to_string(), "plugin".to_string()],
                },
                plugin: PluginInfo {
                    aegis_version: "^0.2.0".to_string(),
                    plugin_api_version: "1.0".to_string(),
                    engine_name: Some("Unity".to_string()),
                    format_support: vec![FileFormatInfo {
                        extension: "unity3d".to_string(),
                        description: "Unity asset bundle".to_string(),
                        mime_type: Some("application/octet-stream".to_string()),
                        tested: true,
                    }],
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

        // Test serialization
        let json = serde_json::to_string(&metadata).unwrap();
        let deserialized: PluginMetadata = serde_json::from_str(&json).unwrap();

        assert_eq!(metadata.name, deserialized.name);
        assert_eq!(metadata.version, deserialized.version);
    }

    #[test]
    fn test_plugin_status_serialization() {
        let status = PluginStatus::Approved;
        let json = serde_json::to_string(&status).unwrap();
        let deserialized: PluginStatus = serde_json::from_str(&json).unwrap();
        assert!(matches!(deserialized, PluginStatus::Approved));
    }
}
