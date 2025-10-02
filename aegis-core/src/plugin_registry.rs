//! Plugin Registry System
//!
//! Centralized system for managing, distributing, and versioning community-contributed
//! format plugins with security-first architecture and compliance integration.

use anyhow::{Result, Context, bail};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use chrono::{DateTime, Utc};
use uuid::Uuid;
use tracing::{info, warn, error, debug};

mod database;
mod models;
mod operations;
#[cfg(feature = "api")]
mod api;
mod manifest;
mod dependency;

pub use database::*;
pub use models::*;
pub use operations::*;
#[cfg(feature = "api")]
pub use api::*;
pub use manifest::*;
pub use dependency::*;

/// Plugin registry configuration
#[derive(Debug, Clone)]
pub struct PluginRegistryConfig {
    /// Database file path
    pub database_path: PathBuf,
    /// Plugin storage directory
    pub plugin_dir: PathBuf,
    /// Enable security scanning
    pub enable_security_scan: bool,
    /// Enable audit logging
    pub enable_audit_log: bool,
    /// Registry API base URL
    pub api_base_url: Option<String>,
}

impl Default for PluginRegistryConfig {
    fn default() -> Self {
        Self {
            database_path: PathBuf::from("./plugin_registry.db"),
            plugin_dir: PathBuf::from("./plugins"),
            enable_security_scan: true,
            enable_audit_log: true,
            api_base_url: None,
        }
    }
}

/// Main plugin registry struct
#[derive(Clone)]
pub struct PluginRegistry {
    config: PluginRegistryConfig,
    operations: PluginOperations,
}

impl PluginRegistry {
    /// Create a new plugin registry with default configuration
    pub fn new() -> Result<Self> {
        Self::with_config(PluginRegistryConfig::default())
    }

    /// Create a new plugin registry with custom configuration
    pub fn with_config(config: PluginRegistryConfig) -> Result<Self> {
        info!("Initializing plugin registry with database: {}", config.database_path.display());

        // Create plugin directory if it doesn't exist
        std::fs::create_dir_all(&config.plugin_dir)
            .context("Failed to create plugin directory")?;

        // Initialize database
        let database = DatabaseConnection::new(&config.database_path)
            .context("Failed to initialize plugin registry database")?;

        // Run migrations
        database.migrate()
            .context("Failed to run database migrations")?;

        let operations = PluginOperations::new(database);

        Ok(Self {
            config,
            operations,
        })
    }

    /// Get registry configuration
    pub fn config(&self) -> &PluginRegistryConfig {
        &self.config
    }

    /// Get plugin operations interface
    pub fn operations(&self) -> &PluginOperations {
        &self.operations
    }

    /// Get plugin operations interface (mutable)
    pub fn operations_mut(&mut self) -> &mut PluginOperations {
        &mut self.operations
    }

    /// Get registry statistics
    pub fn stats(&self) -> Result<RegistryStats> {
        self.operations.stats()
    }

    /// List all available plugins
    pub fn list_plugins(&self) -> Result<Vec<PluginMetadata>> {
        self.operations.list_plugins()
    }

    /// Find a specific plugin by name
    pub fn find_plugin(&self, name: &str) -> Result<Option<PluginMetadata>> {
        self.operations.find_plugin(name)
    }

    /// Get plugin versions
    pub fn get_plugin_versions(&self, plugin_name: &str) -> Result<Vec<PluginVersion>> {
        self.operations.get_plugin_versions(plugin_name)
    }

    /// Get latest version of a plugin
    pub fn get_latest_version(&self, plugin_name: &str) -> Result<Option<PluginVersion>> {
        self.operations.get_latest_version(plugin_name)
    }

    /// Register a new plugin
    pub fn register_plugin(&mut self, metadata: PluginMetadata, package_data: Vec<u8>) -> Result<PluginVersion> {
        info!("Registering plugin: {} v{}", metadata.name, metadata.version);

        self.operations.register_plugin(metadata, package_data)
    }

    /// Update plugin status
    pub fn update_plugin_status(&mut self, plugin_id: &str, status: PluginStatus) -> Result<()> {
        info!("Updating plugin {} status to {:?}", plugin_id, status);
        self.operations.update_plugin_status(plugin_id, status)
    }

    /// Search plugins by criteria
    pub fn search_plugins(&self, criteria: SearchCriteria) -> Result<Vec<PluginMetadata>> {
        self.operations.search_plugins(criteria)
    }

    /// Get download statistics for a plugin
    pub fn get_download_stats(&self, plugin_name: &str) -> Result<DownloadStats> {
        self.operations.get_download_stats(plugin_name)
    }

    /// Record plugin download
    pub fn record_download(&mut self, plugin_name: &str, version: &str, user_id: Option<&str>) -> Result<()> {
        self.operations.record_download(plugin_name, version, user_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_plugin_registry_creation() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_registry.db");

        let config = PluginRegistryConfig {
            database_path: db_path,
            ..Default::default()
        };

        let registry = PluginRegistry::with_config(config);
        assert!(registry.is_ok());
    }

    #[test]
    fn test_plugin_operations() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_ops.db");

        let config = PluginRegistryConfig {
            database_path: db_path,
            ..Default::default()
        };

        let registry = PluginRegistry::with_config(config).unwrap();

        // Test basic operations
        let stats = registry.stats().unwrap();
        assert_eq!(stats.total_plugins, 0);
        assert_eq!(stats.total_versions, 0);
    }
}
