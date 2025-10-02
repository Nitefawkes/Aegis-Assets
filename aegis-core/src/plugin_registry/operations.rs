//! Plugin registry operations
//!
//! Business logic layer for plugin management, search, and registry operations.

use anyhow::{Result, Context, bail};
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use tracing::{info, warn, debug};

use super::{models::*, database::{DatabaseConnection, DatabaseOperations}};

/// Plugin registry operations
#[derive(Clone)]
pub struct PluginOperations {
    database: DatabaseConnection,
}

impl PluginOperations {
    /// Create new operations instance
    pub fn new(database: DatabaseConnection) -> Self {
        Self { database }
    }

    /// Get registry statistics
    pub fn stats(&self) -> Result<RegistryStats> {
        DatabaseOperations::get_stats(&self.database)
    }

    /// List all approved plugins
    pub fn list_plugins(&self) -> Result<Vec<PluginMetadata>> {
        DatabaseOperations::list_plugins(&self.database)
    }

    /// Find a specific plugin by name
    pub fn find_plugin(&self, name: &str) -> Result<Option<PluginMetadata>> {
        DatabaseOperations::find_plugin(&self.database, name)
    }

    /// Get all versions of a plugin
    pub fn get_plugin_versions(&self, plugin_name: &str) -> Result<Vec<PluginVersion>> {
        DatabaseOperations::get_plugin_versions(&self.database, plugin_name)
    }

    /// Get the latest approved version of a plugin
    pub fn get_latest_version(&self, plugin_name: &str) -> Result<Option<PluginVersion>> {
        DatabaseOperations::get_latest_version(&self.database, plugin_name)
    }

    /// Register a new plugin version
    pub fn register_plugin(&mut self, metadata: PluginMetadata, package_data: Vec<u8>) -> Result<PluginVersion> {
        info!("Registering plugin: {} v{}", metadata.name, metadata.version);

        // Validate plugin manifest
        self.validate_plugin_manifest(&metadata)?;

        // Register the plugin version
        let version = DatabaseOperations::register_plugin_version(&mut self.database, &metadata, &package_data)?;

        info!("Successfully registered plugin {} v{}", metadata.name, metadata.version);
        Ok(version)
    }

    /// Update plugin status (admin operation)
    pub fn update_plugin_status(&mut self, plugin_id: &str, status: PluginStatus) -> Result<()> {
        info!("Updating plugin {} status to {:?}", plugin_id, status);

        match status {
            PluginStatus::Approved => {
                // Additional validation for approved plugins
                self.validate_approved_plugin(plugin_id)?;
            }
            PluginStatus::Rejected => {
                warn!("Plugin {} has been rejected", plugin_id);
            }
            PluginStatus::Deprecated => {
                warn!("Plugin {} has been deprecated", plugin_id);
            }
            PluginStatus::Removed => {
                warn!("Plugin {} has been removed", plugin_id);
            }
            PluginStatus::PendingReview => {
                debug!("Plugin {} status reset to pending review", plugin_id);
            }
        }

        DatabaseOperations::update_plugin_status(&mut self.database, plugin_id, status)
    }

    /// Search plugins by criteria
    pub fn search_plugins(&self, criteria: SearchCriteria) -> Result<Vec<PluginMetadata>> {
        debug!("Searching plugins with criteria: {:?}", criteria);

        let plugins = DatabaseOperations::search_plugins(&self.database, criteria)?;

        debug!("Found {} plugins matching search criteria", plugins.len());
        Ok(plugins)
    }

    /// Get download statistics for a plugin
    pub fn get_download_stats(&self, plugin_name: &str) -> Result<DownloadStats> {
        DatabaseOperations::get_download_stats(&self.database, plugin_name)
    }

    /// Record a plugin download
    pub fn record_download(&mut self, plugin_name: &str, version: &str, user_id: Option<&str>) -> Result<()> {
        debug!("Recording download for plugin {} v{}", plugin_name, version);

        DatabaseOperations::record_download(&mut self.database, plugin_name, version, user_id)?;

        info!("Download recorded for plugin {} v{}", plugin_name, version);
        Ok(())
    }

    /// Get plugin by ID
    pub fn get_plugin_by_id(&self, id: &str) -> Result<Option<PluginMetadata>> {
        DatabaseOperations::get_plugin_by_id(&self.database, id)
    }

    /// Get plugin version by ID
    pub fn get_plugin_version_by_id(&self, id: &str) -> Result<Option<PluginVersion>> {
        DatabaseOperations::get_plugin_version_by_id(&self.database, id)
    }

    /// Update plugin metadata
    pub fn update_plugin_metadata(&mut self, plugin_id: &str, metadata: &PluginMetadata) -> Result<()> {
        debug!("Updating metadata for plugin {}", plugin_id);

        DatabaseOperations::update_plugin_metadata(&mut self.database, plugin_id, metadata)
    }

    /// Delete a plugin version
    pub fn delete_plugin_version(&mut self, plugin_name: &str, version: &str) -> Result<()> {
        warn!("Deleting plugin version {} v{}", plugin_name, version);

        DatabaseOperations::delete_plugin_version(&mut self.database, plugin_name, version)?;

        info!("Deleted plugin version {} v{}", plugin_name, version);
        Ok(())
    }

    /// Get plugins by author
    pub fn get_plugins_by_author(&self, author_email: &str) -> Result<Vec<PluginMetadata>> {
        DatabaseOperations::get_plugins_by_author(&self.database, author_email)
    }

    /// Validate plugin manifest
    fn validate_plugin_manifest(&self, metadata: &PluginMetadata) -> Result<()> {
        // Validate required fields
        if metadata.name.is_empty() {
            bail!("Plugin name cannot be empty");
        }

        if metadata.version.is_empty() {
            bail!("Plugin version cannot be empty");
        }

        if metadata.author_email.is_empty() {
            bail!("Plugin author email cannot be empty");
        }

        if metadata.license.is_empty() {
            bail!("Plugin license cannot be empty");
        }

        // Validate email format
        if !metadata.author_email.contains('@') {
            bail!("Plugin author email must be valid email address");
        }

        // Validate version format (basic semantic versioning)
        if !self.is_valid_version(&metadata.version) {
            bail!("Plugin version must follow semantic versioning (e.g., 1.0.0, 1.0.0-alpha.1)");
        }

        // Validate Aegis version requirement
        if metadata.manifest.plugin.aegis_version.is_empty() {
            bail!("Plugin must specify compatible Aegis version requirement");
        }

        // Validate plugin API version
        if metadata.manifest.plugin.plugin_api_version.is_empty() {
            bail!("Plugin must specify plugin API version requirement");
        }

        // Validate format support
        if metadata.manifest.plugin.format_support.is_empty() {
            bail!("Plugin must declare at least one supported file format");
        }

        for format in &metadata.manifest.plugin.format_support {
            if format.extension.is_empty() {
                bail!("File format extension cannot be empty");
            }

            if format.description.is_empty() {
                bail!("File format description cannot be empty");
            }
        }

        // Validate dependencies
        for (dep_name, dep_version) in &metadata.manifest.dependencies {
            if dep_name.is_empty() {
                bail!("Dependency name cannot be empty");
            }

            if dep_version.is_empty() {
                bail!("Dependency version requirement cannot be empty");
            }
        }

        debug!("Plugin manifest validation passed for {}", metadata.name);
        Ok(())
    }

    /// Validate that a plugin is ready for approval
    fn validate_approved_plugin(&self, plugin_id: &str) -> Result<()> {
        let version = self.database.get_plugin_version_by_id(plugin_id)?
            .ok_or_else(|| anyhow::anyhow!("Plugin version not found: {}", plugin_id))?;

        // Check for security scan requirement
        if let Some(security_config) = &version.manifest.security {
            match security_config.vulnerability_scan {
                super::ScanRequirement::Required => {
                    // Check if security scan has been performed
                    // This would be implemented when security scanning is added
                    warn!("Security scan validation not yet implemented for plugin {}", plugin_id);
                }
                super::ScanRequirement::Optional => {
                    debug!("Security scan is optional for plugin {}", plugin_id);
                }
                super::ScanRequirement::Disabled => {
                    debug!("Security scan is disabled for plugin {}", plugin_id);
                }
            }
        }

        // Check for code signing requirement
        if let Some(security_config) = &version.manifest.security {
            match security_config.code_signing_cert {
                super::CodeSigningLevel::Community => {
                    if version.signature.is_none() {
                        warn!("Plugin {} should have community code signing", plugin_id);
                    }
                }
                super::CodeSigningLevel::Verified => {
                    if version.signature.is_none() {
                        bail!("Plugin {} requires verified code signing", plugin_id);
                    }
                }
                super::CodeSigningLevel::Enterprise => {
                    if version.signature.is_none() {
                        bail!("Plugin {} requires enterprise code signing", plugin_id);
                    }
                }
            }
        }

        debug!("Plugin approval validation passed for {}", plugin_id);
        Ok(())
    }

    /// Validate semantic version format
    fn is_valid_version(&self, version: &str) -> bool {
        // Basic semantic versioning check: MAJOR.MINOR.PATCH(-PRERELEASE)(+BUILD)
        let version_regex = regex::Regex::new(r"^(\d+)\.(\d+)\.(\d+)(?:-([a-zA-Z0-9\-\.]+))?(?:\+([a-zA-Z0-9\-\.]+))?$")
            .expect("Invalid version regex");

        version_regex.is_match(version)
    }

    /// Get plugins pending review (admin function)
    pub fn get_pending_plugins(&self) -> Result<Vec<PluginVersion>> {
        let versions = self.database.with_connection(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT pv.id, pv.plugin_id, pv.version, pv.status, pv.package_size,
                       pv.package_hash, pv.manifest, pv.signature_data, pv.published_at
                FROM plugin_versions pv
                WHERE pv.status = 'pending_review'
                ORDER BY pv.published_at ASC
                "#
            )?;

            let versions = stmt.query_map([], |row| {
                let manifest_str: String = row.get(6)?;
                let manifest: PluginManifest = serde_json::from_str(&manifest_str)?;

                let signature_data: Option<String> = row.get(7)?;
                let signature = signature_data.and_then(|s| serde_json::from_str(&s).ok());

                Ok(PluginVersion {
                    id: row.get(0)?,
                    plugin_id: row.get(1)?,
                    version: row.get(2)?,
                    status: serde_json::from_str(&row.get::<_, String>(3)?)?,
                    package_size: row.get(4)?,
                    package_hash: row.get(5)?,
                    manifest,
                    signature,
                    published_at: row.get(8)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()
            .context("Failed to query pending plugins")?;

            Ok(versions)
        })?;

        Ok(versions)
    }

    /// Get security scan results for a plugin version
    pub fn get_security_scan_results(&self, plugin_version_id: &str) -> Result<Vec<SecurityScanResult>> {
        self.database.with_connection(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT id, plugin_version_id, scan_timestamp, scan_status,
                       vulnerability_count, scan_report, scanner_version
                FROM plugin_security_scans
                WHERE plugin_version_id = ?
                ORDER BY scan_timestamp DESC
                "#
            )?;

            let scans = stmt.query_map([plugin_version_id], |row| {
                let scan_report_str: String = row.get(5)?;
                let scan_report: serde_json::Value = serde_json::from_str(&scan_report_str)?;

                Ok(SecurityScanResult {
                    id: row.get(0)?,
                    plugin_version_id: row.get(1)?,
                    scan_timestamp: row.get(2)?,
                    scan_status: serde_json::from_str(&row.get::<_, String>(3)?)?,
                    vulnerability_count: row.get(4)?,
                    scanner_version: row.get(6)?,
                    scan_report,
                })
            })?
            .collect::<Result<Vec<_>, _>>()
            .context("Failed to query security scan results")?;

            Ok(scans)
        })?
    }

    /// Get plugin reviews and ratings
    pub fn get_plugin_reviews(&self, plugin_id: &str) -> Result<(PluginRating, Vec<PluginReview>)> {
        self.database.with_connection(|conn| {
            // Get rating summary
            let rating_stats: (f64, i64) = conn.query_row(
                r#"
                SELECT AVG(rating), COUNT(*)
                FROM plugin_reviews
                WHERE plugin_id = ?
                "#,
                [plugin_id],
                |row| Ok((row.get(0)?, row.get(1)?))
            ).context("Failed to get rating stats")?;

            let mut distribution = HashMap::new();
            let mut stmt = conn.prepare(
                r#"
                SELECT rating, COUNT(*) as count
                FROM plugin_reviews
                WHERE plugin_id = ?
                GROUP BY rating
                "#
            )?;

            let rating_rows = stmt.query_map([plugin_id], |row| {
                Ok((row.get::<_, u8>(0)?, row.get::<_, i64>(1)? as usize))
            })?;

            for rating_row in rating_rows {
                let (rating, count) = rating_row?;
                distribution.insert(rating, count);
            }

            let rating = PluginRating {
                average: rating_stats.0,
                count: rating_stats.1 as usize,
                distribution,
            };

            // Get recent reviews
            let mut stmt = conn.prepare(
                r#"
                SELECT id, user_id, rating, review_text, version_used,
                       helpful_count, created_at, updated_at
                FROM plugin_reviews
                WHERE plugin_id = ?
                ORDER BY created_at DESC
                LIMIT 10
                "#
            )?;

            let reviews = stmt.query_map([plugin_id], |row| {
                Ok(PluginReview {
                    id: row.get(0)?,
                    user_id: row.get(1)?,
                    rating: row.get(2)?,
                    review_text: row.get(3)?,
                    version_used: row.get(4)?,
                    helpful_count: row.get(5)?,
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()
            .context("Failed to query reviews")?;

            Ok((rating, reviews))
        })?
    }

    /// Get audit log entries
    pub fn get_audit_log(&self, limit: Option<usize>, offset: Option<usize>) -> Result<Vec<AuditLogEntry>> {
        self.database.with_connection(|conn| {
            let mut query = String::from(
                r#"
                SELECT id, table_name, record_id, operation, old_values, new_values,
                       changed_by, change_timestamp, ip_address, user_agent
                FROM plugin_audit_log
                ORDER BY change_timestamp DESC
                "#
            );

            let mut params = Vec::new();

            if let Some(limit_val) = limit {
                query.push_str(" LIMIT ?");
                params.push(limit_val.to_string());
            }

            if let Some(offset_val) = offset {
                query.push_str(" OFFSET ?");
                params.push(offset_val.to_string());
            }

            let mut stmt = conn.prepare(&query)?;
            let audit_entries = stmt.query_map(&params.iter().map(|s| s.as_str()).collect::<Vec<_>>(), |row| {
                let old_values_str: Option<String> = row.get(4)?;
                let old_values = old_values_str.and_then(|s| serde_json::from_str(&s).ok());

                let new_values_str: Option<String> = row.get(5)?;
                let new_values = new_values_str.and_then(|s| serde_json::from_str(&s).ok());

                Ok(AuditLogEntry {
                    id: row.get(0)?,
                    table_name: row.get(1)?,
                    record_id: row.get(2)?,
                    operation: serde_json::from_str(&row.get::<_, String>(3)?)?,
                    old_values,
                    new_values,
                    changed_by: row.get(6)?,
                    change_timestamp: row.get(7)?,
                    ip_address: row.get(8)?,
                    user_agent: row.get(9)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()
            .context("Failed to query audit log")?;

            Ok(audit_entries)
        })?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::models::*;
    use tempfile::TempDir;

    fn create_test_database() -> Result<(DatabaseConnection, PluginOperations)> {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_ops.db");

        let database = DatabaseConnection::new(&db_path)?;
        database.migrate()?;

        let operations = PluginOperations::new(database);
        Ok((database, operations))
    }

    #[test]
    fn test_plugin_operations_creation() {
        let (_db, ops) = create_test_database().unwrap();
        let stats = ops.stats().unwrap();

        assert_eq!(stats.total_plugins, 0);
        assert_eq!(stats.total_versions, 0);
        assert_eq!(stats.total_downloads, 0);
    }

    #[test]
    fn test_plugin_manifest_validation() {
        let (_db, ops) = create_test_database().unwrap();

        // Create invalid plugin metadata (empty name)
        let invalid_metadata = PluginMetadata {
            id: "test".to_string(),
            name: "".to_string(), // Invalid: empty name
            display_name: "Test Plugin".to_string(),
            description: Some("Test".to_string()),
            author_email: "test@example.com".to_string(),
            license: "MIT".to_string(),
            homepage: None,
            repository: None,
            keywords: vec!["test".to_string()],
            created_at: Utc::now(),
            updated_at: Utc::now(),
            version: "1.0.0".to_string(),
            status: PluginStatus::PendingReview,
            package_size: 1024,
            package_hash: "test".to_string(),
            manifest: PluginManifest {
                package: PackageInfo {
                    name: "test".to_string(),
                    version: "1.0.0".to_string(),
                    description: Some("Test".to_string()),
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

        // Test validation failure
        let result = ops.validate_plugin_manifest(&invalid_metadata);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Plugin name cannot be empty"));
    }

    #[test]
    fn test_version_validation() {
        let (_db, ops) = create_test_database().unwrap();

        assert!(ops.is_valid_version("1.0.0"));
        assert!(ops.is_valid_version("1.2.3"));
        assert!(ops.is_valid_version("1.0.0-alpha.1"));
        assert!(ops.is_valid_version("1.0.0-beta.2"));
        assert!(ops.is_valid_version("2.0.0+build.1"));

        assert!(!ops.is_valid_version(""));
        assert!(!ops.is_valid_version("1.0"));
        assert!(!ops.is_valid_version("1.0.0.0"));
        assert!(!ops.is_valid_version("invalid"));
    }
}
