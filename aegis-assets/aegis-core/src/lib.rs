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
//! let result = extractor.extract_from_file(
//!     Path::new("game.unity3d"),
//!     Path::new("./output/")
//! )?;
//!
//! println!("Extracted {} assets", result.resources.len());
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

pub mod archive;
pub mod audit;
pub mod compliance;
pub mod events;
pub mod export;
pub mod extract;
pub mod patch;
pub mod resource;

// Re-export commonly used types
pub use archive::{
    ArchiveHandler, ComplianceLevel, ComplianceProfile, ComplianceRegistry, EntryId, EntryMetadata,
    FormatSupport, PluginInfo, Provenance,
};
pub use events::{
    ExtractionEvent, ExtractionEventEmitter, ExtractionEventKind, JobState, NoopEventEmitter,
};
pub use export::{ExportFormat, ExportOptions, Exporter};
pub use extract::{ExtractionError, ExtractionResult, Extractor};
pub use patch::{PatchRecipe, PatchRecipeBuilder};
pub use resource::{
    AnimationResource, AudioResource, BlendMode, LevelResource, LoopMode, MaterialResource,
    MeshResource, Resource, TextureFormat, TextureResource, TextureUsage,
};

use anyhow::Result;
use std::collections::HashMap;
use tracing::info;
use tracing_subscriber::EnvFilter;

/// Version information for the core library
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const GIT_HASH: &str = match option_env!("VERGEN_GIT_SHA") {
    Some(hash) => hash,
    None => "unknown",
};

/// Initialize the Aegis-Core library with logging and telemetry
pub fn init() -> Result<()> {
    // Initialize tracing subscriber for structured logging
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("aegis_core=info,aegis_plugins=info"));

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(false)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .init();

    info!("Initializing Aegis-Assets Core v{}", VERSION);
    info!("Git commit: {}", GIT_HASH);

    Ok(())
}

/// Plugin registry for managing available format handlers
pub struct PluginRegistry {
    handlers: HashMap<String, Box<dyn PluginFactory>>,
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
    fn compliance_info(&self) -> PluginInfo;
}

impl PluginRegistry {
    /// Create a new plugin registry
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    /// Register a plugin factory
    pub fn register_plugin(&mut self, factory: Box<dyn PluginFactory>) {
        let name = factory.name().to_string();
        info!("Registering plugin: {} v{}", name, factory.version());
        self.handlers.insert(name, factory);
    }

    /// Find a suitable plugin for the given file
    pub fn find_handler(&self, path: &std::path::Path, bytes: &[u8]) -> Option<&dyn PluginFactory> {
        // First try extension-based detection
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            for factory in self.handlers.values() {
                if factory.supported_extensions().contains(&ext) && factory.can_handle(bytes) {
                    return Some(factory.as_ref());
                }
            }
        }

        // Fall back to content-based detection
        for factory in self.handlers.values() {
            if factory.can_handle(bytes) {
                return Some(factory.as_ref());
            }
        }

        None
    }

    /// Get all registered plugins
    pub fn list_plugins(&self) -> Vec<&dyn PluginFactory> {
        self.handlers.values().map(|f| f.as_ref()).collect()
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

        Ok(Self {
            config: Config::default(),
            plugin_registry: PluginRegistry::new(),
            compliance_registry: ComplianceRegistry::new(),
        })
    }

    /// Create Aegis-Core with custom configuration
    pub fn with_config(config: Config) -> Result<Self> {
        init()?;

        Ok(Self {
            config,
            plugin_registry: PluginRegistry::new(),
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
        Extractor::with_registries(
            &self.plugin_registry,
            &self.compliance_registry,
            self.config.clone(),
        )
    }

    /// Get system information
    pub fn system_info(&self) -> SystemInfo {
        SystemInfo {
            version: VERSION.to_string(),
            git_hash: GIT_HASH.to_string(),
            registered_plugins: self.plugin_registry.handlers.len(),
            compliance_profiles: self.compliance_registry.len(),
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
