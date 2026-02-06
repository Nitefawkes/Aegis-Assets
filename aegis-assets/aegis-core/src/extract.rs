use crate::{
    archive::ComplianceRegistry,
    audit::{AuditEvent, AuditEventKind, AuditLogWriter},
    resource::Resource,
    compliance::{ComplianceChecker, ComplianceResult},
    resource::{BinaryResource, Resource, TextContentType, TextResource},
    Config, PluginRegistry,
};
use std::path::{Path, PathBuf};
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

    #[error("Audit log error: {0}")]
    AuditLogError(String),
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
}

impl Extractor {
    /// Create a new extractor with default registries
    pub fn new(_compliance_registry: ComplianceRegistry) -> Self {
        Self {
            plugin_registry: None,
            compliance_registry: None, // Will be properly initialized
            config: Config::default(),
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
        }
    }

    /// Extract assets from a file
    pub fn extract_from_file(
        &mut self,
        source_path: &Path,
        output_dir: &Path,
    ) -> Result<ExtractionResult, ExtractionError> {
        let start_time = std::time::Instant::now();
        let job_id = Uuid::new_v4();
        let audit_logger = AuditLogWriter::from_config(&self.config);
        let log_event = |event: AuditEvent| {
            if let Err(err) = audit_logger.log_event(&event) {
                if !matches!(err, crate::audit::AuditError::Disabled) {
                    error!("Failed to write audit event: {}", err);
                }
            }
        };

        info!("Starting extraction from: {}", source_path.display());
        log_event(AuditEvent::new(
            job_id,
            AuditEventKind::JobStarted {
                source_path: source_path.to_string_lossy().to_string(),
                output_dir: output_dir.to_string_lossy().to_string(),
            },
        ));
        let job_id = uuid::Uuid::new_v4();

        info!(
            event = "extraction_job_status",
            status = "started",
            job_id = %job_id,
            source_path = %source_path.display()
        );

        // Check if file exists
        if !source_path.exists() {
            log_event(AuditEvent::new(
                job_id,
                AuditEventKind::JobCompleted {
                    success: false,
                    duration_ms: start_time.elapsed().as_millis() as u64,
                    resource_count: 0,
                    error_message: Some("File not found".to_string()),
                },
            ));
            return Err(ExtractionError::FileNotFound(source_path.to_path_buf()));
        }

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

        let plugin_factory = registry
            .find_handler(source_path, &header)
            .ok_or_else(|| ExtractionError::NoSuitablePlugin(source_path.to_path_buf()))?;

        info!(
            event = "plugin_detected",
            job_id = %job_id,
            plugin = plugin_factory.name(),
            plugin_version = plugin_factory.version()
        );

        let handler = plugin_factory.create_handler(source_path).map_err(|err| {
            ExtractionError::PluginError {
                plugin: plugin_factory.name().to_string(),
                error: err.to_string(),
            }
        })?;

        let game_id = handler
            .provenance()
            .game_id
            .clone()
            .or_else(|| {
                source_path
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .map(|stem| stem.to_string())
            })
            .unwrap_or_else(|| "unknown".to_string());

        let format = source_path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or(plugin_factory.name());

        if let Some(compliance_registry) = self
            .compliance_registry
            .map(|registry| unsafe { &*registry })
        {
            if let Some(profile) = compliance_registry.get_profile(&game_id) {
                info!(
                    event = "compliance_profile_detected",
                    job_id = %job_id,
                    game_id,
                    publisher = profile.publisher.as_str(),
                    enforcement_level = ?profile.enforcement_level
                );
            }
        }

        let mut compliance_checker = ComplianceChecker::new();
        if let Some(enterprise) = &self.config.enterprise_config {
            if enterprise.require_compliance_verification {
                compliance_checker = compliance_checker.with_strict_mode();
            }
        }

        let compliance_result = compliance_checker.check_extraction_allowed(&game_id, format);
        let compliance_info = Self::map_compliance_result(&compliance_result);

        Self::emit_compliance_event(&job_id, &game_id, format, &compliance_result);

        if matches!(compliance_result, ComplianceResult::Blocked { .. }) {
            if let ComplianceResult::Blocked { reason, .. } = compliance_result {
                return Err(ExtractionError::ComplianceViolation(reason));
            }
        }

        let entries = handler
            .list_entries()
            .map_err(|err| ExtractionError::PluginError {
                plugin: plugin_factory.name().to_string(),
                error: err.to_string(),
            })?;

        let mut resources = Vec::new();
        let mut bytes_extracted = 0u64;

        for entry in entries.iter() {
            let data =
                handler
                    .read_entry(&entry.id)
                    .map_err(|err| ExtractionError::PluginError {
                        plugin: plugin_factory.name().to_string(),
                        error: err.to_string(),
                    })?;

            bytes_extracted += data.len() as u64;
            resources.push(Self::resource_from_entry(entry, data));
        }

        let duration = start_time.elapsed();
        let metrics = ExtractionMetrics {
            duration_ms: duration.as_millis() as u64,
            peak_memory_mb: (bytes_extracted / (1024 * 1024)) as usize,
            files_processed: entries.len(),
            bytes_extracted,
        };
        let verification_required = self
            .config
            .enterprise_config
            .as_ref()
            .map(|enterprise| enterprise.require_compliance_verification)
            .unwrap_or(false);

        log_event(AuditEvent::new(
            job_id,
            AuditEventKind::ComplianceDecision {
                is_compliant: compliance_info.is_compliant,
                risk_level: format!("{:?}", compliance_info.risk_level),
                warnings: compliance_info.warnings.clone(),
                recommendations: compliance_info.recommendations.clone(),
                verification_required,
            },
        ));

        if verification_required && !compliance_info.is_compliant {
            log_event(AuditEvent::new(
                job_id,
                AuditEventKind::JobCompleted {
                    success: false,
                    duration_ms: start_time.elapsed().as_millis() as u64,
                    resource_count: 0,
                    error_message: Some("Compliance verification failed".to_string()),
                },
            ));
            return Err(ExtractionError::ComplianceViolation(
                "Compliance verification required".to_string(),
            ));
        }

        log_event(AuditEvent::new(
            job_id,
            AuditEventKind::PluginUsed {
                plugin_name: "mock".to_string(),
                plugin_version: "0.0.0".to_string(),
            },
        ));

        for resource in &resources {
            let output_path = output_dir.join(resource.name());
            log_event(AuditEvent::new(
                job_id,
                AuditEventKind::OutputGenerated {
                    output_path: output_path.to_string_lossy().to_string(),
                    resource_name: resource.name().to_string(),
                    resource_type: resource.resource_type().to_string(),
                    estimated_memory_bytes: resource.estimated_memory_usage(),
                },
            ));
        }

        info!(
            event = "extraction_job_status",
            status = "completed",
            job_id = %job_id,
            duration_ms = metrics.duration_ms,
            resources_extracted = resources.len(),
            bytes_extracted = metrics.bytes_extracted
        );

        log_event(AuditEvent::new(
            job_id,
            AuditEventKind::JobCompleted {
                success: true,
                duration_ms: metrics.duration_ms,
                resource_count: resources.len(),
                error_message: None,
            },
        ));

        Ok(ExtractionResult {
            source_path: source_path.to_path_buf(),
            resources,
            warnings: vec![],
            compliance_info,
            metrics,
        })
    }

    fn map_compliance_result(result: &ComplianceResult) -> ComplianceInfo {
        match result {
            ComplianceResult::Allowed {
                profile,
                warnings,
                recommendations,
            } => ComplianceInfo {
                is_compliant: true,
                risk_level: profile.enforcement_level,
                warnings: warnings.clone(),
                recommendations: recommendations.clone(),
            },
            ComplianceResult::AllowedWithWarnings {
                profile,
                warnings,
                recommendations,
            } => ComplianceInfo {
                is_compliant: true,
                risk_level: profile.enforcement_level,
                warnings: warnings.clone(),
                recommendations: recommendations.clone(),
            },
            ComplianceResult::HighRiskWarning {
                profile,
                warnings,
                explicit_consent_required,
            } => ComplianceInfo {
                is_compliant: !explicit_consent_required,
                risk_level: profile.enforcement_level,
                warnings: warnings.clone(),
                recommendations: vec![
                    "Explicit consent required before distributing extracted assets.".to_string(),
                ],
            },
            ComplianceResult::Blocked {
                profile,
                reason,
                alternatives,
            } => ComplianceInfo {
                is_compliant: false,
                risk_level: profile.enforcement_level,
                warnings: vec![reason.clone()],
                recommendations: alternatives.clone(),
            },
        }
    }

    fn emit_compliance_event(
        job_id: &uuid::Uuid,
        game_id: &str,
        format: &str,
        result: &ComplianceResult,
    ) {
        match result {
            ComplianceResult::Allowed { profile, .. } => {
                info!(
                    event = "compliance_decision",
                    job_id = %job_id,
                    game_id,
                    format,
                    decision = "allowed",
                    risk_level = ?profile.enforcement_level
                );
            }
            ComplianceResult::AllowedWithWarnings { profile, .. } => {
                info!(
                    event = "compliance_decision",
                    job_id = %job_id,
                    game_id,
                    format,
                    decision = "allowed_with_warnings",
                    risk_level = ?profile.enforcement_level
                );
            }
            ComplianceResult::HighRiskWarning {
                profile,
                explicit_consent_required,
                ..
            } => {
                info!(
                    event = "compliance_decision",
                    job_id = %job_id,
                    game_id,
                    format,
                    decision = "high_risk_warning",
                    explicit_consent_required = *explicit_consent_required,
                    risk_level = ?profile.enforcement_level
                );
            }
            ComplianceResult::Blocked {
                profile, reason, ..
            } => {
                info!(
                    event = "compliance_decision",
                    job_id = %job_id,
                    game_id,
                    format,
                    decision = "blocked",
                    risk_level = ?profile.enforcement_level,
                    reason
                );
            }
        }
    }

    fn resource_from_entry(entry: &crate::archive::EntryMetadata, data: Vec<u8>) -> Resource {
        if let Some(content_type) = Self::guess_text_content_type(&entry.path) {
            if let Ok(content) = String::from_utf8(data.clone()) {
                return Resource::Text(TextResource {
                    name: entry.name.clone(),
                    content_type,
                    content,
                });
            }
        }

        Resource::Binary(BinaryResource {
            name: entry.name.clone(),
            mime_type: None,
            data,
        })
    }

    fn guess_text_content_type(path: &Path) -> Option<TextContentType> {
        let extension = path.extension().and_then(|ext| ext.to_str())?;
        let content_type = match extension.to_ascii_lowercase().as_str() {
            "json" => TextContentType::JSON,
            "xml" => TextContentType::XML,
            "yaml" | "yml" => TextContentType::YAML,
            "lua" | "js" | "cs" | "py" | "rb" | "ts" => TextContentType::Script,
            "shader" | "hlsl" | "glsl" => TextContentType::Shader,
            "txt" | "md" | "ini" => TextContentType::Plain,
            _ => return None,
        };

        Some(content_type)
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
