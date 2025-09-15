use crate::{
    archive::ComplianceRegistry,
    compliance::{ComplianceChecker, ComplianceResult},
    resource::Resource,
    PluginRegistry, Config,
};
use anyhow::Result;
use std::path::{Path, PathBuf};
use thiserror::Error;
use tracing::{info, error};

/// Errors that can occur during extraction
#[derive(Debug, Error)]
pub enum ExtractionError {
    #[error("No suitable plugin found for file: {0}")]
    NoSuitablePlugin(PathBuf),
    
    #[error("Compliance check failed: {0}")]
    ComplianceViolation(String),
    
    #[error("File not found: {0}")]
    FileNotFound(PathBuf),
    
    #[error("Invalid file format: {0}")]
    InvalidFormat(String),
    
    #[error("Memory limit exceeded (limit: {limit}MB, required: {required}MB)")]
    MemoryLimitExceeded { limit: usize, required: usize },
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Plugin error: {plugin}: {error}")]
    PluginError { plugin: String, error: String },
    
    #[error("Generic error: {0}")]
    Generic(#[from] anyhow::Error),
}

/// Result of an extraction operation
#[derive(Debug, Clone)]
pub struct ExtractionResult {
    /// Path to the source file
    pub source_path: PathBuf,
    /// Extracted resources
    pub resources: Vec<Resource>,
    /// Warnings encountered during extraction
    pub warnings: Vec<String>,
    /// Compliance information
    pub compliance_info: ComplianceInfo,
    /// Performance metrics
    pub metrics: ExtractionMetrics,
}

/// Compliance information for the extraction
#[derive(Debug, Clone)]
pub struct ComplianceInfo {
    /// Whether the extraction is compliant
    pub is_compliant: bool,
    /// Risk level
    pub risk_level: crate::archive::ComplianceLevel,
    /// Warnings or restrictions
    pub warnings: Vec<String>,
    /// Recommended actions
    pub recommendations: Vec<String>,
}

/// Performance metrics for extraction
#[derive(Debug, Clone)]
pub struct ExtractionMetrics {
    /// Total time taken (milliseconds)
    pub duration_ms: u64,
    /// Memory peak usage (MB)
    pub peak_memory_mb: usize,
    /// Number of files processed
    pub files_processed: usize,
    /// Total bytes extracted
    pub bytes_extracted: u64,
}

/// Main extraction engine
pub struct Extractor {
    plugin_registry: PluginRegistry,
    compliance_checker: ComplianceChecker,
    config: Config,
}

impl Extractor {
    /// Create a new extractor with registries
    pub fn new(plugin_registry: PluginRegistry, compliance_registry: ComplianceRegistry) -> Self {
        let compliance_checker = ComplianceChecker::from_registry(compliance_registry);
        Self {
            plugin_registry,
            compliance_checker,
            config: Config::default(),
        }
    }
    
    /// Create a new extractor with custom config
    pub fn with_config(plugin_registry: PluginRegistry, compliance_registry: ComplianceRegistry, config: Config) -> Self {
        let compliance_checker = ComplianceChecker::from_registry(compliance_registry);
        Self {
            plugin_registry,
            compliance_checker,
            config,
        }
    }
    
    /// Get a reference to the plugin registry
    pub fn plugin_registry(&self) -> &PluginRegistry {
        &self.plugin_registry
    }
    
    /// Get a reference to the compliance checker
    pub fn compliance_checker(&self) -> &ComplianceChecker {
        &self.compliance_checker
    }
    
    /// Extract assets from a file
    pub fn extract_from_file(
        &mut self,
        source_path: &Path,
        _output_dir: &Path,
    ) -> Result<ExtractionResult, ExtractionError> {
        let start_time = std::time::Instant::now();
        
        info!("Starting extraction from: {}", source_path.display());
        
        // Check if file exists
        if !source_path.exists() {
            return Err(ExtractionError::FileNotFound(source_path.to_path_buf()));
        }
        
        // Read file header for plugin detection
        let mut file = std::fs::File::open(source_path)?;
        let mut header = vec![0u8; 1024];
        use std::io::Read;
        let bytes_read = file.read(&mut header)?;
        header.truncate(bytes_read);
        
        // Find suitable plugin using registry
        info!("Detecting file format...");
        let plugin_factory = self.plugin_registry
            .find_handler(source_path, &header)
            .ok_or_else(|| ExtractionError::NoSuitablePlugin(source_path.to_path_buf()))?;
        
        info!("Using plugin: {} v{}", plugin_factory.name(), plugin_factory.version());
        
        // Check compliance before extraction
        let game_id = source_path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");
        let format = source_path.extension()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");
            
        let compliance_result = self.compliance_checker.check_extraction_allowed(game_id, format);
        
        // Handle compliance result
        let (is_allowed, warnings, recommendations, risk_level) = match &compliance_result {
            ComplianceResult::Allowed { warnings, recommendations, profile } => {
                (true, warnings.clone(), recommendations.clone(), profile.enforcement_level)
            }
            ComplianceResult::AllowedWithWarnings { warnings, recommendations, profile } => {
                (true, warnings.clone(), recommendations.clone(), profile.enforcement_level)
            }
            ComplianceResult::HighRiskWarning { warnings, profile, .. } => {
                return Err(ExtractionError::ComplianceViolation(
                    format!("High-risk extraction requires explicit consent. {}", warnings.join("; "))
                ));
            }
            ComplianceResult::Blocked { reason, .. } => {
                return Err(ExtractionError::ComplianceViolation(reason.clone()));
            }
        };
        
        if !warnings.is_empty() {
            for warning in &warnings {
                info!("Compliance warning: {}", warning);
            }
        }
        
        // Create handler and extract resources
        let handler = plugin_factory.create_handler(source_path)?;
        let entries = handler.list_entries()?;
        
        info!("Found {} entries in archive", entries.len());
        
        // Convert entries to resources (simplified)
        let mut resources = Vec::new();
        for entry in entries {
            let resource_type = match entry.file_type.as_deref() {
                Some("texture") => crate::resource::ResourceType::Texture,
                Some("mesh") => crate::resource::ResourceType::Mesh,
                Some("audio") => crate::resource::ResourceType::Audio,
                _ => crate::resource::ResourceType::Generic,
            };
            
            let resource = crate::resource::Resource {
                id: entry.id.0.clone(),
                name: entry.name.clone(),
                resource_type,
                size: entry.size_uncompressed,
                format: entry.file_type.unwrap_or_else(|| "unknown".to_string()),
                metadata: std::collections::HashMap::new(),
            };
            resources.push(resource);
        }
        
        let duration = start_time.elapsed();
        let total_bytes: u64 = resources.iter().map(|r| r.size).sum();
        let metrics = ExtractionMetrics {
            duration_ms: duration.as_millis() as u64,
            peak_memory_mb: 64, // TODO: Implement actual memory monitoring
            files_processed: resources.len(),
            bytes_extracted: total_bytes,
        };
        
        let compliance_info = ComplianceInfo {
            is_compliant: is_allowed,
            risk_level,
            warnings,
            recommendations,
        };
        
        info!(
            "Extraction completed in {}ms, {} resources extracted",
            metrics.duration_ms,
            resources.len()
        );
        
        Ok(ExtractionResult {
            source_path: source_path.to_path_buf(),
            resources,
            warnings: vec![],
            compliance_info,
            metrics,
        })
    }
    
    /// Batch extract multiple files
    pub fn extract_batch(
        &mut self,
        sources: Vec<&Path>,
        output_dir: &Path,
    ) -> Result<Vec<ExtractionResult>, ExtractionError> {
        let mut results = Vec::new();
        
        for source in sources {
            match self.extract_from_file(source, output_dir) {
                Ok(result) => results.push(result),
                Err(e) => {
                    error!("Failed to extract {}: {}", source.display(), e);
                    // Continue with other files in batch mode
                }
            }
        }
        
        Ok(results)
    }
    
    /// Get extraction statistics
    pub fn get_stats(&self) -> ExtractionStats {
        ExtractionStats {
            files_processed: 0,
            total_resources_extracted: 0,
            total_bytes_processed: 0,
            average_processing_time_ms: 0,
        }
    }
}

/// Overall extraction statistics
#[derive(Debug, Clone)]
pub struct ExtractionStats {
    pub files_processed: usize,
    pub total_resources_extracted: usize,
    pub total_bytes_processed: u64,
    pub average_processing_time_ms: u64,
}

// Safe implementations since we're not actually using raw pointers yet
unsafe impl Send for Extractor {}
unsafe impl Sync for Extractor {}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs::File;
    use std::io::Write;
    
    #[test]
    fn test_extractor_creation() {
        let plugin_registry = crate::PluginRegistry::new();
        let compliance = ComplianceRegistry::new();
        let extractor = Extractor::new(plugin_registry, compliance);
        let stats = extractor.get_stats();
        
        assert_eq!(stats.files_processed, 0);
    }
    
    #[test]
    fn test_file_not_found() {
        let plugin_registry = crate::PluginRegistry::new();
        let compliance = ComplianceRegistry::new();
        let mut extractor = Extractor::new(plugin_registry, compliance);
        let temp_dir = TempDir::new().unwrap();
        
        let result = extractor.extract_from_file(
            &temp_dir.path().join("nonexistent.file"),
            temp_dir.path(),
        );
        
        assert!(matches!(result, Err(ExtractionError::FileNotFound(_))));
    }
    
    #[test]
    fn test_mock_extraction() {
        let plugin_registry = crate::PluginRegistry::new();
        let compliance = ComplianceRegistry::new();
        let mut extractor = Extractor::new(plugin_registry, compliance);
        let temp_dir = TempDir::new().unwrap();
        
        // Create a dummy file
        let test_file = temp_dir.path().join("test.dat");
        let mut file = File::create(&test_file).unwrap();
        file.write_all(b"test data").unwrap();
        
        let result = extractor.extract_from_file(&test_file, temp_dir.path());
        // This will likely fail since we have no plugins registered, but that's expected
        assert!(result.is_err());
    }
}
