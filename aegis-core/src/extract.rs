use crate::{
    archive::{ComplianceRegistry, ConvertedEntry, EntryMetadata},
    compliance::{ComplianceChecker, ComplianceResult},
    resource::Resource,
    Config, PluginRegistry,
};
use anyhow::Result;
use std::fs;
use std::path::{Component, Path, PathBuf};
use thiserror::Error;
use tracing::{error, info, warn};

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
    /// Base directory where raw/converted assets were written
    pub output_dir: PathBuf,
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
    pub fn with_config(
        plugin_registry: PluginRegistry,
        compliance_registry: ComplianceRegistry,
        config: Config,
    ) -> Self {
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
        output_dir: &Path,
    ) -> Result<ExtractionResult, ExtractionError> {
        let start_time = std::time::Instant::now();

        info!("Starting extraction from: {}", source_path.display());

        // Check if file exists
        if !source_path.exists() {
            return Err(ExtractionError::FileNotFound(source_path.to_path_buf()));
        }

        fs::create_dir_all(output_dir)?;

        // Read file header for plugin detection
        let mut file = std::fs::File::open(source_path)?;
        let mut header = vec![0u8; 1024];
        use std::io::Read;
        let bytes_read = file.read(&mut header)?;
        header.truncate(bytes_read);

        // Find suitable plugin using registry
        info!("Detecting file format...");
        let plugin_factory = self
            .plugin_registry
            .find_handler(source_path, &header)
            .ok_or_else(|| ExtractionError::NoSuitablePlugin(source_path.to_path_buf()))?;

        info!(
            "Using plugin: {} v{}",
            plugin_factory.name(),
            plugin_factory.version()
        );
        let plugin_name = plugin_factory.name().to_string();

        // Check compliance before extraction
        let game_id = source_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");
        let format = source_path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");

        let compliance_result = self
            .compliance_checker
            .check_extraction_allowed(game_id, format);

        // Handle compliance result
        let (is_allowed, warnings, recommendations, risk_level) = match &compliance_result {
            ComplianceResult::Allowed {
                warnings,
                recommendations,
                profile,
            } => (
                true,
                warnings.clone(),
                recommendations.clone(),
                profile.enforcement_level,
            ),
            ComplianceResult::AllowedWithWarnings {
                warnings,
                recommendations,
                profile,
            } => (
                true,
                warnings.clone(),
                recommendations.clone(),
                profile.enforcement_level,
            ),
            ComplianceResult::HighRiskWarning { warnings, .. } => {
                return Err(ExtractionError::ComplianceViolation(format!(
                    "High-risk extraction requires explicit consent. {}",
                    warnings.join("; ")
                )));
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
        let handler = plugin_factory.create_handler(source_path).map_err(|e| {
            ExtractionError::PluginError {
                plugin: plugin_name.clone(),
                error: e.to_string(),
            }
        })?;
        let entries = handler
            .list_entries()
            .map_err(|e| ExtractionError::PluginError {
                plugin: plugin_name.clone(),
                error: e.to_string(),
            })?;

        info!("Found {} entries in archive", entries.len());

        // Convert entries to resources (simplified)
        let mut resources = Vec::new();
        let mut extraction_warnings = Vec::new();
        let mut total_bytes = 0u64;

        for entry in entries {
            let resource_type = match entry.file_type.as_deref() {
                Some("texture") => crate::resource::ResourceType::Texture,
                Some("mesh") => crate::resource::ResourceType::Mesh,
                Some("audio") => crate::resource::ResourceType::Audio,
                _ => crate::resource::ResourceType::Generic,
            };

            let mut resource = Resource::new(
                entry.id.0.clone(),
                entry.name.clone(),
                resource_type,
                entry.size_uncompressed,
                entry
                    .file_type
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string()),
            );

            resource.metadata.insert(
                "archive_entry_path".to_string(),
                entry.path.display().to_string(),
            );

            let raw_bytes =
                handler
                    .read_entry(&entry.id)
                    .map_err(|e| ExtractionError::PluginError {
                        plugin: plugin_name.clone(),
                        error: e.to_string(),
                    })?;

            let relative_path = sanitize_entry_path(&entry);
            let raw_output_path = output_dir.join(&relative_path);
            if let Some(parent) = raw_output_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&raw_output_path, &raw_bytes)?;

            resource.set_raw_path(&raw_output_path);
            resource.metadata.insert(
                "raw_output_path".to_string(),
                raw_output_path.display().to_string(),
            );
            resource.size = raw_bytes.len() as u64;
            total_bytes += raw_bytes.len() as u64;

            match handler.read_converted_entry(&entry.id) {
                Ok(Some(ConvertedEntry {
                    filename,
                    data,
                    converted,
                })) => {
                    if converted {
                        let converted_dir = output_dir.join("converted");
                        fs::create_dir_all(&converted_dir)?;
                        let converted_output_path = converted_dir.join(&filename);
                        fs::write(&converted_output_path, &data)?;
                        resource.add_converted_path(&converted_output_path);
                        resource.metadata.insert(
                            format!(
                                "converted_output_{}",
                                resource.converted_output_paths().len()
                            ),
                            converted_output_path.display().to_string(),
                        );
                    }
                }
                Ok(None) => {}
                Err(e) => {
                    warn!(
                        "Failed to produce converted output for {}: {}",
                        entry.name, e
                    );
                    extraction_warnings.push(format!(
                        "Failed to convert {} via plugin {}: {}",
                        entry.name, plugin_name, e
                    ));
                }
            }

            resources.push(resource);
        }

        let duration = start_time.elapsed();
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
            output_dir: output_dir.to_path_buf(),
            resources,
            warnings: extraction_warnings,
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

fn sanitize_entry_path(entry: &EntryMetadata) -> PathBuf {
    let candidate = if entry.path.as_os_str().is_empty() {
        PathBuf::from(entry.name.clone())
    } else {
        entry.path.clone()
    };

    let mut sanitized = PathBuf::new();
    for component in candidate.components() {
        match component {
            Component::Normal(part) => sanitized.push(part),
            Component::CurDir => {}
            Component::RootDir | Component::Prefix(_) | Component::ParentDir => {}
        }
    }

    if sanitized.as_os_str().is_empty() {
        sanitized.push(&entry.name);
    }

    sanitized
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
    use crate::archive::{
        ArchiveHandler, ComplianceLevel, ComplianceProfile, ConvertedEntry, EntryId, EntryMetadata,
        PluginInfo, Provenance,
    };
    use crate::PluginFactory;
    use chrono::Utc;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;
    use uuid::Uuid;

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

        let result =
            extractor.extract_from_file(&temp_dir.path().join("nonexistent.file"), temp_dir.path());

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

    #[test]
    fn extraction_writes_outputs() {
        let mut plugin_registry = crate::PluginRegistry::new();
        plugin_registry.register_plugin(Box::new(MockFactory));
        let compliance = ComplianceRegistry::new();
        let mut extractor = Extractor::new(plugin_registry, compliance);
        let temp_dir = TempDir::new().unwrap();

        let source = temp_dir.path().join("bundle.mock");
        std::fs::write(&source, b"MOCKDATA").unwrap();
        let output_dir = temp_dir.path().join("out");

        let result = extractor
            .extract_from_file(&source, &output_dir)
            .expect("extraction should succeed");

        assert_eq!(result.resources.len(), 1);
        assert_eq!(result.output_dir, output_dir);

        let resource = &result.resources[0];
        let raw_path = resource.raw_output_path().expect("raw output path");
        assert!(raw_path.exists());
        assert!(raw_path.starts_with(&result.output_dir));
        assert_eq!(resource.size, MOCK_RAW.len() as u64);

        let converted = resource.converted_output_paths();
        assert_eq!(converted.len(), 1);
        assert!(converted[0].exists());
    }

    const MOCK_RAW: &[u8] = &[42u8; 64];

    struct MockFactory;

    impl PluginFactory for MockFactory {
        fn name(&self) -> &str {
            "Mock"
        }

        fn version(&self) -> &str {
            "1.0.0"
        }

        fn supported_extensions(&self) -> Vec<&str> {
            vec!["mock"]
        }

        fn can_handle(&self, bytes: &[u8]) -> bool {
            bytes.starts_with(b"MOCK") || !bytes.is_empty()
        }

        fn create_handler(&self, path: &Path) -> Result<Box<dyn ArchiveHandler>> {
            Ok(Box::new(MockHandler::new(path)?))
        }

        fn compliance_info(&self) -> PluginInfo {
            PluginInfo {
                name: "Mock".to_string(),
                version: "1.0.0".to_string(),
                author: Some("Test".to_string()),
                compliance_verified: true,
            }
        }
    }

    struct MockHandler {
        compliance_profile: ComplianceProfile,
        provenance: Provenance,
    }

    impl MockHandler {
        fn new(path: &Path) -> Result<Self> {
            let compliance_profile = ComplianceProfile {
                publisher: "Mock Publisher".to_string(),
                game_id: Some("mock_game".to_string()),
                enforcement_level: ComplianceLevel::Permissive,
                official_support: true,
                bounty_eligible: false,
                enterprise_warning: None,
                mod_policy_url: None,
                supported_formats: std::collections::HashMap::new(),
            };

            let source_bytes = std::fs::read(path)?;
            let provenance = Provenance {
                session_id: Uuid::new_v4(),
                game_id: Some("mock_game".to_string()),
                source_hash: blake3::hash(&source_bytes).to_hex().to_string(),
                source_path: path.to_path_buf(),
                compliance_profile: compliance_profile.clone(),
                extraction_time: Utc::now(),
                aegis_version: crate::VERSION.to_string(),
                plugin_info: PluginInfo {
                    name: "Mock".to_string(),
                    version: "1.0.0".to_string(),
                    author: Some("Test".to_string()),
                    compliance_verified: true,
                },
            };

            Ok(Self {
                compliance_profile,
                provenance,
            })
        }
    }

    impl ArchiveHandler for MockHandler {
        fn detect(bytes: &[u8]) -> bool {
            !bytes.is_empty()
        }

        fn open(path: &Path) -> Result<Self>
        where
            Self: Sized,
        {
            Self::new(path)
        }

        fn compliance_profile(&self) -> &ComplianceProfile {
            &self.compliance_profile
        }

        fn list_entries(&self) -> Result<Vec<EntryMetadata>> {
            Ok(vec![EntryMetadata {
                id: EntryId::new("entry_1"),
                name: "MockTexture".to_string(),
                path: PathBuf::from("textures/mock_texture.bin"),
                size_compressed: None,
                size_uncompressed: MOCK_RAW.len() as u64,
                file_type: Some("texture".to_string()),
                last_modified: None,
                checksum: None,
            }])
        }

        fn read_entry(&self, _id: &EntryId) -> Result<Vec<u8>> {
            Ok(MOCK_RAW.to_vec())
        }

        fn read_converted_entry(&self, _id: &EntryId) -> Result<Option<ConvertedEntry>> {
            Ok(Some(ConvertedEntry {
                filename: "mock_texture.png".to_string(),
                data: b"converted".to_vec(),
                converted: true,
            }))
        }

        fn provenance(&self) -> &Provenance {
            &self.provenance
        }
    }
}
