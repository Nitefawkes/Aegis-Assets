//! # Aegis-Assets Core
//! 
//! Compliance-first platform for game asset extraction, preservation, and creative workflows.
//! 
//! This crate provides the core engine for Aegis-Assets, including:
//! - Plugin architecture for extensible format support
//! - Compliance-aware extraction with risk assessment
//! - Resource type definitions for common game assets
//! - Export capabilities to modern formats (glTF, KTX2, OGG)
//! 
//! ## Architecture
//! 
//! Aegis-Core is built around a plugin architecture where each game engine format
//! is supported by implementing the `ArchiveHandler` trait. This allows for:
//! 
//! - **Extensibility**: New formats can be added without modifying core code
//! - **Compliance**: Each plugin declares its legal risk level and compliance status
//! - **Performance**: Rust's zero-cost abstractions enable high-performance extraction
//! - **Safety**: Memory safety and error handling built into the type system
//! 
//! ## Quick Start
//! 
//! ```rust,no_run
//! use aegis_core::{
//!     archive::{ArchiveHandler, ComplianceRegistry},
//!     extract::Extractor,
//! };
//! use std::path::Path;
//! 
//! // Load compliance profiles
//! let compliance = ComplianceRegistry::load_from_directory(
//!     Path::new("compliance-profiles")
//! )?;
//! 
//! // Create extractor with compliance checking
//! let mut extractor = Extractor::new(compliance);
//! 
//! // Extract assets (automatically detects format and applies compliance)
//! let results = extractor.extract_from_file(
//!     Path::new("game.unity3d"),
//!     Path::new("./output/")
//! )?;
//! 
//! println!("Extracted {} assets", results.len());
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

pub mod archive;
pub mod resource;
pub mod export;
pub mod asset_db;
pub mod extract;
pub mod compliance;
pub mod patch;
pub mod security;

#[cfg(feature = "registry-api")]
pub mod plugin_registry;

#[cfg(feature = "api")]
pub mod api;
#[cfg(test)]
pub mod test_integration;

// Re-export commonly used types
pub use archive::{
    ArchiveHandler, ComplianceLevel, ComplianceProfile, ComplianceRegistry,
    EntryId, EntryMetadata, FormatSupport, Provenance,
};
pub use resource::{
    Resource, MeshResource, TextureResource, MaterialResource, 
    AnimationResource, AudioResource, LevelResource,
    TextureFormat, TextureUsage, BlendMode, LoopMode,
};
pub use extract::{Extractor, ExtractionResult, ExtractionError};
pub use export::{ExportFormat, ExportOptions, Exporter};
pub use patch::{PatchRecipe, PatchRecipeBuilder};
pub use security::{SecurityManager, SecurityReport, ThreatLevel, ComplianceStatus};

// Plugin system exports (defined in this module)

use anyhow::Result;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tracing::info;

/// Plugin information for CLI and API consumers
#[derive(Debug, Clone)]
pub struct PluginInfo {
    name: String,
    version: String,
    supported_extensions: Vec<String>,
}

impl PluginInfo {
    pub fn new(name: String, version: String, supported_extensions: Vec<String>) -> Self {
        Self {
            name,
            version,
            supported_extensions,
        }
    }
    
    pub fn name(&self) -> &str {
        &self.name
    }
    
    pub fn version(&self) -> &str {
        &self.version
    }
    
    pub fn supported_extensions(&self) -> &[String] {
        &self.supported_extensions
    }
    
    /// Mock compliance info for CLI compatibility
    pub fn compliance_info(&self) -> ComplianceInfo {
        ComplianceInfo {
            risk_level: "Low".to_string(),
            legal_status: "Safe".to_string(),
            notes: "Community-maintained plugin".to_string(),
            compliance_verified: true,
            description: Some(format!("{} plugin for Aegis-Assets", self.name)),
            author: Some("Aegis-Assets Team".to_string()),
            homepage: Some("https://github.com/aegis-assets/aegis-assets".to_string()),
            publisher_policy: "Permissive".to_string(),
            bounty_eligible: true,
            enterprise_approved: true,
        }
    }
}

/// Compliance information for plugins
#[derive(Debug, Clone)]
pub struct ComplianceInfo {
    pub risk_level: String,
    pub legal_status: String,
    pub notes: String,
    pub compliance_verified: bool,
    pub description: Option<String>,
    pub author: Option<String>,
    pub homepage: Option<String>,
    pub publisher_policy: String,
    pub bounty_eligible: bool,
    pub enterprise_approved: bool,
}

/// Version information for the core library
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const GIT_HASH: &str = match option_env!("VERGEN_GIT_SHA") {
    Some(hash) => hash,
    None => "unknown",
};

/// Initialize the Aegis-Core library with logging and telemetry
pub fn init() -> Result<()> {
    // Initialize tracing subscriber for structured logging
    let _ = tracing_subscriber::fmt()
        .with_env_filter("aegis_core=info,aegis_plugins=info")
        .with_target(false)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .try_init();

    info!("Initializing Aegis-Assets Core v{}", VERSION);
    info!("Git commit: {}", GIT_HASH);
    
    Ok(())
}

/// Plugin registry for managing available format handlers
pub struct PluginRegistry {
    handlers: Arc<RwLock<HashMap<String, Box<dyn PluginFactory>>>>,
    #[cfg(feature = "registry-api")]
    database: Option<Arc<plugin_registry::database::DatabaseConnection>>,
}

impl Clone for PluginRegistry {
    fn clone(&self) -> Self {
        Self {
            handlers: Arc::clone(&self.handlers),
            #[cfg(feature = "registry-api")]
            database: self.database.clone(),
        }
    }
}

/// Factory trait for creating archive handlers
pub trait PluginFactory: Send + Sync {
    /// Get the name of this plugin
    fn name(&self) -> &str;
    
    /// Get plugin version
    fn version(&self) -> &str;
    
    /// Get supported file extensions
    fn supported_extensions(&self) -> Vec<&str>;
    
    /// Check if this plugin can handle the given file
    fn can_handle(&self, bytes: &[u8]) -> bool;
    
    /// Create a new archive handler instance
    fn create_handler(&self, path: &std::path::Path) -> Result<Box<dyn ArchiveHandler>>;
    
    /// Get compliance information for this plugin
    fn compliance_info(&self) -> crate::PluginInfo;
}

impl PluginRegistry {
    /// Create a new plugin registry
    pub fn new() -> Self {
        Self {
            handlers: Arc::new(RwLock::new(HashMap::new())),
            #[cfg(feature = "registry-api")]
            database: None,
        }
    }
    
    /// Create a new plugin registry with database support
    #[cfg(feature = "registry-api")]
    pub fn with_database(db_path: &std::path::Path) -> Result<Self> {
        let database = plugin_registry::database::DatabaseConnection::new(db_path)?;
        database.migrate()?;
        Ok(Self {
            handlers: Arc::new(RwLock::new(HashMap::new())),
            database: Some(Arc::new(database)),
        })
    }
    
    /// Load default plugins (Unity, etc.)
    pub fn load_default_plugins() -> Self {
        let mut registry = Self::new();
        registry.discover_and_register_plugins();
        registry
    }
    
    /// Automatically discover and register available plugins
    pub fn discover_and_register_plugins(&mut self) -> Result<()> {
        info!("Discovering available plugins...");
        
        // Register Unity plugin if available
        #[cfg(feature = "unity-plugin")]
        {
            info!("Unity plugin feature enabled, attempting to register Unity plugin");
            use aegis_unity_plugin::UnityPluginFactory;
            self.register_plugin(Box::new(UnityPluginFactory));
        }
        
        // Register Unreal plugin if available
        #[cfg(feature = "unreal-plugin")]
        {
            info!("Unreal plugin feature enabled, attempting to register Unreal plugin");
            use aegis_unreal_plugin::UnrealPluginFactory;
            self.register_plugin(Box::new(UnrealPluginFactory));
        }
        
        // TODO: Add dynamic plugin discovery from directories
        // This would scan for plugin shared libraries and load them
        
        let handlers = self.handlers.read().unwrap();
        info!("Plugin discovery complete. Registered {} plugins", handlers.len());
        
        Ok(())
    }
    
    /// Register a plugin factory
    pub fn register_plugin(&mut self, factory: Box<dyn PluginFactory>) {
        let name = factory.name().to_string();
        let version = factory.version().to_string();
        info!("Registering plugin: {} v{}", name, version);
        
        let mut handlers = self.handlers.write().unwrap();
        handlers.insert(name, factory);
    }
    
    /// Find a suitable plugin for the given file
    pub fn find_handler(&self, path: &std::path::Path, bytes: &[u8]) -> Option<Box<dyn ArchiveHandler>> {
        if let Some((handler, _info)) = self.find_handler_with_info(path, bytes) {
            Some(handler)
        } else {
            None
        }
    }
    
    /// Find a suitable plugin and return both handler and plugin info
    pub fn find_handler_with_info(&self, path: &std::path::Path, bytes: &[u8]) -> Option<(Box<dyn ArchiveHandler>, PluginInfo)> {
        let handlers = self.handlers.read().unwrap();
        
        // First try extension-based detection
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            for factory in handlers.values() {
                if factory.supported_extensions().contains(&ext) && factory.can_handle(bytes) {
                    if let Ok(handler) = factory.create_handler(path) {
                        let plugin_info = PluginInfo::new(
                            factory.name().to_string(), 
                            factory.version().to_string(),
                            factory.supported_extensions().iter().map(|s| s.to_string()).collect()
                        );
                        return Some((handler, plugin_info));
                    }
                }
            }
        }
        
        // Fall back to content-based detection
        for factory in handlers.values() {
            if factory.can_handle(bytes) {
                if let Ok(handler) = factory.create_handler(path) {
                    let plugin_info = PluginInfo::new(
                        factory.name().to_string(), 
                        factory.version().to_string(),
                        factory.supported_extensions().iter().map(|s| s.to_string()).collect()
                    );
                    return Some((handler, plugin_info));
                }
            }
        }
        
        None
    }
    
    /// Get a factory by name (for testing and specific use cases)
    pub fn get_factory(&self, name: &str) -> Option<Box<dyn ArchiveHandler>> {
        let handlers = self.handlers.read().unwrap();
        if let Some(factory) = handlers.get(name) {
            // We can't return a factory reference due to lifetime issues,
            // but we can create a handler for testing
            None // Placeholder - would need path
        } else {
            None
        }
    }
    
    /// Get all registered plugin information
    pub fn list_plugins(&self) -> Vec<PluginInfo> {
        let handlers = self.handlers.read().unwrap();
        handlers.values()
            .map(|f| PluginInfo::new(
                f.name().to_string(), 
                f.version().to_string(),
                f.supported_extensions().iter().map(|s| s.to_string()).collect()
            ))
            .collect()
    }
    
    /// Get count of registered plugins
    pub fn plugin_count(&self) -> usize {
        let handlers = self.handlers.read().unwrap();
        handlers.len()
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Global configuration for Aegis-Core
#[derive(Debug, Clone)]
pub struct Config {
    /// Maximum memory usage for extraction (in MB)
    pub max_memory_mb: usize,
    /// Enable parallel processing
    pub enable_parallel: bool,
    /// Temporary directory for extraction
    pub temp_dir: Option<std::path::PathBuf>,
    /// Enable AI features (requires additional dependencies)
    pub enable_ai_features: bool,
    /// Enterprise features configuration
    pub enterprise_config: Option<EnterpriseConfig>,
}

/// Enterprise-specific configuration
#[derive(Debug, Clone)]
pub struct EnterpriseConfig {
    /// Enable audit logging
    pub enable_audit_logs: bool,
    /// Audit log directory
    pub audit_log_dir: std::path::PathBuf,
    /// Require compliance verification
    pub require_compliance_verification: bool,
    /// Steam API key for library verification
    pub steam_api_key: Option<String>,
    /// Epic Games API key for library verification
    pub epic_api_key: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_memory_mb: 4096, // 4GB default
            enable_parallel: true,
            temp_dir: None, // Use system temp
            enable_ai_features: false,
            enterprise_config: None,
        }
    }
}

/// Main entry point for the Aegis-Assets system
pub struct AegisCore {
    config: Config,
    plugin_registry: PluginRegistry,
    compliance_registry: ComplianceRegistry,
}

impl AegisCore {
    /// Create a new Aegis-Core instance with default configuration
    pub fn new() -> Result<Self> {
        init()?;
        
        let mut plugin_registry = PluginRegistry::new();
        plugin_registry.discover_and_register_plugins()?;
        
        Ok(Self {
            config: Config::default(),
            plugin_registry,
            compliance_registry: ComplianceRegistry::new(),
        })
    }
    
    /// Create Aegis-Core with custom configuration
    pub fn with_config(config: Config) -> Result<Self> {
        init()?;
        
        let mut plugin_registry = PluginRegistry::new();
        plugin_registry.discover_and_register_plugins()?;
        
        Ok(Self {
            config,
            plugin_registry,
            compliance_registry: ComplianceRegistry::new(),
        })
    }
    
    /// Load compliance profiles from directory
    pub fn load_compliance_profiles(&mut self, dir: &std::path::Path) -> Result<()> {
        info!("Loading compliance profiles from: {}", dir.display());
        self.compliance_registry = ComplianceRegistry::load_from_directory(dir)?;
        Ok(())
    }
    
    /// Register a plugin
    pub fn register_plugin(&mut self, factory: Box<dyn PluginFactory>) {
        self.plugin_registry.register_plugin(factory);
    }
    
    /// Create an extractor instance
    pub fn create_extractor(&self) -> Extractor {
        Extractor::with_config(
            self.plugin_registry.clone(),
            self.compliance_registry.clone(),
            self.config.clone(),
        )
    }

    /// Create an extractor instance with security enabled
    pub async fn create_secure_extractor(&self) -> Result<Extractor> {
        let mut extractor = Extractor::with_config(
            self.plugin_registry.clone(),
            self.compliance_registry.clone(),
            self.config.clone(),
        );
        
        // Initialize security framework if enterprise features are enabled
        if self.config.enterprise_config.is_some() {
            extractor.init_security().await?;
        }
        
        Ok(extractor)
    }
    
    /// Get system information
    pub fn system_info(&self) -> SystemInfo {
        SystemInfo {
            version: VERSION.to_string(),
            git_hash: GIT_HASH.to_string(),
            registered_plugins: self.plugin_registry.plugin_count(),
            compliance_profiles: self.compliance_registry.profile_count(),
            config: self.config.clone(),
        }
    }
}

/// System information for debugging and diagnostics
#[derive(Debug, Clone)]
pub struct SystemInfo {
    pub version: String,
    pub git_hash: String,
    pub registered_plugins: usize,
    pub compliance_profiles: usize,
    pub config: Config,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_aegis_core_creation() {
        let core = AegisCore::new().expect("Failed to create AegisCore");
        let info = core.system_info();
        
        assert_eq!(info.version, VERSION);
        assert_eq!(info.registered_plugins, 0); // No plugins registered yet
    }
    
    #[test]
    fn test_plugin_registry() {
        let mut registry = PluginRegistry::new();
        assert_eq!(registry.list_plugins().len(), 0);
        
        // Plugin registration would be tested with actual plugin implementations
    }
}
