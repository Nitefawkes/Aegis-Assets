use crate::{
    archive::ComplianceRegistry,
    events::{
        ExtractionEvent, ExtractionEventEmitter, ExtractionEventKind, JobState, NoopEventEmitter,
    },
    resource::Resource,
    Config, PluginRegistry,
};
use chrono::Utc;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use thiserror::Error;
use tracing::{error, info};
use uuid::Uuid;

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
    plugin_registry: Option<*const PluginRegistry>,
    compliance_registry: Option<*const ComplianceRegistry>,
    config: Config,
    event_emitter: Arc<dyn ExtractionEventEmitter>,
}

impl Extractor {
    /// Create a new extractor with default registries
    pub fn new(_compliance_registry: ComplianceRegistry) -> Self {
        Self {
            plugin_registry: None,
            compliance_registry: None, // Will be properly initialized
            config: Config::default(),
            event_emitter: Arc::new(NoopEventEmitter),
        }
    }

    /// Create extractor with specific registries
    pub fn with_registries(
        plugin_registry: &PluginRegistry,
        compliance_registry: &ComplianceRegistry,
        config: Config,
    ) -> Self {
        Self {
            plugin_registry: Some(plugin_registry as *const _),
            compliance_registry: Some(compliance_registry as *const _),
            config,
            event_emitter: Arc::new(NoopEventEmitter),
        }
    }

    /// Extract assets from a file
    pub fn extract_from_file(
        &mut self,
        source_path: &Path,
        _output_dir: &Path,
    ) -> Result<ExtractionResult, ExtractionError> {
        let job_id = Uuid::new_v4();
        self.extract_from_file_with_job_id(source_path, _output_dir, job_id)
    }

    /// Extract assets from a file using a known job ID
    pub fn extract_from_file_with_job_id(
        &mut self,
        source_path: &Path,
        _output_dir: &Path,
        job_id: Uuid,
    ) -> Result<ExtractionResult, ExtractionError> {
        let start_time = std::time::Instant::now();
        let job_id = uuid::Uuid::new_v4();

        self.emit_event(ExtractionEvent {
            job_id,
            occurred_at: Utc::now(),
            kind: ExtractionEventKind::JobStateChange {
                state: JobState::Queued,
                message: Some("Extraction requested.".to_string()),
            },
        });

        info!("Starting extraction from: {}", source_path.display());

        // Check if file exists
        if !source_path.exists() {
            self.emit_event(ExtractionEvent {
                job_id,
                occurred_at: Utc::now(),
                kind: ExtractionEventKind::JobStateChange {
                    state: JobState::Failed,
                    message: Some("Source file not found.".to_string()),
                },
            });
            return Err(ExtractionError::FileNotFound(source_path.to_path_buf()));
        }

        self.emit_event(ExtractionEvent {
            job_id,
            occurred_at: Utc::now(),
            kind: ExtractionEventKind::JobStateChange {
                state: JobState::Running,
                message: Some("Detecting file format.".to_string()),
            },
        });

        // Read file header for plugin detection
        let mut file = std::fs::File::open(source_path)?;
        let mut header = vec![0u8; 1024];
        use std::io::Read;
        let bytes_read = file.read(&mut header)?;
        header.truncate(bytes_read);

        // Find suitable plugin via registry
        info!(
            event = "plugin_detection_started",
            job_id = %job_id,
            source_path = %source_path.display()
        );

        let registry = self.plugin_registry.map(|registry| unsafe { &*registry });
        let registry =
            registry.ok_or_else(|| ExtractionError::NoSuitablePlugin(source_path.to_path_buf()))?;

        // For now, create a mock result
        let resources = self.mock_extract_resources(source_path)?;
        let total_resources = resources.len();
        for (index, _resource) in resources.iter().enumerate() {
            self.emit_event(ExtractionEvent {
                job_id,
                occurred_at: Utc::now(),
                kind: ExtractionEventKind::AssetIndexingProgress {
                    indexed: index + 1,
                    total: total_resources,
                },
            });
        }

        let duration = start_time.elapsed();
        let metrics = ExtractionMetrics {
            duration_ms: duration.as_millis() as u64,
            peak_memory_mb: (bytes_extracted / (1024 * 1024)) as usize,
            files_processed: entries.len(),
            bytes_extracted,
        };

        self.emit_event(ExtractionEvent {
            job_id,
            occurred_at: Utc::now(),
            kind: ExtractionEventKind::ComplianceDecision {
                is_compliant: compliance_info.is_compliant,
                risk_level: compliance_info.risk_level,
                warnings: compliance_info.warnings.clone(),
                recommendations: compliance_info.recommendations.clone(),
            },
        });

        info!(
            event = "extraction_job_status",
            status = "completed",
            job_id = %job_id,
            duration_ms = metrics.duration_ms,
            resources_extracted = resources.len(),
            bytes_extracted = metrics.bytes_extracted
        );

        self.emit_event(ExtractionEvent {
            job_id,
            occurred_at: Utc::now(),
            kind: ExtractionEventKind::JobStateChange {
                state: JobState::Completed,
                message: Some("Extraction completed.".to_string()),
            },
        });

        Ok(ExtractionResult {
            source_path: source_path.to_path_buf(),
            resources,
            warnings: vec![],
            compliance_info,
            metrics,
        })
    }

    pub fn set_event_emitter(&mut self, event_emitter: Arc<dyn ExtractionEventEmitter>) {
        self.event_emitter = event_emitter;
    }

    fn emit_event(&self, event: ExtractionEvent) {
        self.event_emitter.emit(event);
    }

    /// Mock resource extraction for initial implementation
    fn mock_extract_resources(
        &self,
        _source_path: &Path,
    ) -> Result<Vec<Resource>, ExtractionError> {
        // This is a placeholder - real implementation would use plugins
        let texture = Resource::Texture(crate::resource::TextureResource {
            name: "mock_texture.png".to_string(),
            width: 256,
            height: 256,
            format: crate::resource::TextureFormat::RGBA8,
            data: vec![255u8; 256 * 256 * 4], // White texture
            mip_levels: 1,
            usage_hint: Some(crate::resource::TextureUsage::Albedo),
        });

        Ok(vec![texture])
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
    use crate::archive::{ArchiveHandler, EntryId, EntryMetadata, PluginInfo, Provenance};
    use crate::PluginFactory;
    use anyhow::Result;
    use std::fs::File;
    use std::io::Write;
    use std::path::PathBuf;
    use tempfile::TempDir;

    struct TestPluginFactory;

    struct TestArchive {
        data: Vec<u8>,
        entries: Vec<EntryMetadata>,
        compliance_profile: crate::archive::ComplianceProfile,
        provenance: Provenance,
    }

    impl PluginFactory for TestPluginFactory {
        fn name(&self) -> &str {
            "TestPlugin"
        }

        fn version(&self) -> &str {
            "0.1.0"
        }

        fn supported_extensions(&self) -> Vec<&str> {
            vec!["dat"]
        }

        fn can_handle(&self, _bytes: &[u8]) -> bool {
            true
        }

        fn create_handler(&self, path: &std::path::Path) -> Result<Box<dyn ArchiveHandler>> {
            Ok(Box::new(TestArchive::open(path)?))
        }

        fn compliance_info(&self) -> PluginInfo {
            PluginInfo {
                name: "TestPlugin".to_string(),
                version: "0.1.0".to_string(),
                author: Some("Test".to_string()),
                compliance_verified: true,
            }
        }
    }

    impl ArchiveHandler for TestArchive {
        fn detect(_bytes: &[u8]) -> bool
        where
            Self: Sized,
        {
            true
        }

        fn open(path: &std::path::Path) -> Result<Self>
        where
            Self: Sized,
        {
            let data = std::fs::read(path)?;
            let file_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("test.dat")
                .to_string();

            let entry = EntryMetadata {
                id: EntryId::new("entry_0"),
                name: file_name.clone(),
                path: PathBuf::from(path),
                size_compressed: None,
                size_uncompressed: data.len() as u64,
                file_type: path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|s| s.to_string()),
                last_modified: None,
                checksum: None,
            };

            let compliance_profile = ComplianceRegistry::default_profile();
            let provenance = Provenance {
                session_id: uuid::Uuid::new_v4(),
                game_id: Some("test_game".to_string()),
                source_hash: blake3::hash(&data).to_hex().to_string(),
                source_path: path.to_path_buf(),
                compliance_profile: compliance_profile.clone(),
                extraction_time: chrono::Utc::now(),
                aegis_version: crate::VERSION.to_string(),
                plugin_info: PluginInfo {
                    name: "TestPlugin".to_string(),
                    version: "0.1.0".to_string(),
                    author: Some("Test".to_string()),
                    compliance_verified: true,
                },
            };

            Ok(Self {
                data,
                entries: vec![entry],
                compliance_profile,
                provenance,
            })
        }

        fn compliance_profile(&self) -> &crate::archive::ComplianceProfile {
            &self.compliance_profile
        }

        fn list_entries(&self) -> Result<Vec<EntryMetadata>> {
            Ok(self.entries.clone())
        }

        fn read_entry(&self, _id: &EntryId) -> Result<Vec<u8>> {
            Ok(self.data.clone())
        }

        fn provenance(&self) -> &Provenance {
            &self.provenance
        }
    }

    #[test]
    fn test_extractor_creation() {
        let compliance = ComplianceRegistry::new();
        let extractor = Extractor::new(compliance);
        let stats = extractor.get_stats();

        assert_eq!(stats.files_processed, 0);
    }

    #[test]
    fn test_file_not_found() {
        let compliance = ComplianceRegistry::new();
        let mut extractor = Extractor::new(compliance);
        let temp_dir = TempDir::new().unwrap();

        let result =
            extractor.extract_from_file(&temp_dir.path().join("nonexistent.file"), temp_dir.path());

        assert!(matches!(result, Err(ExtractionError::FileNotFound(_))));
    }

    #[test]
    fn test_mock_extraction() {
        let compliance = ComplianceRegistry::new();
        let mut registry = PluginRegistry::new();
        registry.register_plugin(Box::new(TestPluginFactory));
        let mut extractor = Extractor::with_registries(&registry, &compliance, Config::default());
        let temp_dir = TempDir::new().unwrap();

        // Create a dummy file
        let test_file = temp_dir.path().join("test.dat");
        let mut file = File::create(&test_file).unwrap();
        file.write_all(b"test data").unwrap();

        let result = extractor.extract_from_file(&test_file, temp_dir.path());
        assert!(result.is_ok());

        let extraction_result = result.unwrap();
        assert_eq!(extraction_result.resources.len(), 1);
        assert!(extraction_result.compliance_info.is_compliant);
    }
}
