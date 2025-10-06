use crate::{
    archive::ComplianceRegistry,
    compliance::{ComplianceChecker, ComplianceResult},
    resource::Resource,
    security::{SecurityManager, SecurityReport},
    Config, PluginRegistry,
};
use std::sync::Arc;
use anyhow::Result;
use std::path::{Path, PathBuf};
use thiserror::Error;
use tracing::{error, info, warn};

#[cfg(test)]
use once_cell::sync::Lazy;
#[cfg(test)]
use std::{collections::VecDeque, sync::Mutex};

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
    /// Security report (if security framework is enabled)
    pub security_report: Option<SecurityReport>,
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
///
/// The peak memory metric is gathered directly from the operating system and
/// therefore reflects platform-specific behavior. See the documentation on the
/// `peak_memory_mb` field for details.
#[derive(Debug, Clone)]
pub struct ExtractionMetrics {
    /// Total time taken (milliseconds)
    pub duration_ms: u64,
    /// Peak resident memory usage observed for the process (MB).
    ///
    /// On Unix platforms this reads `ru_maxrss` via `getrusage`, which reports
    /// usage in kibibytes on Linux/Android and in bytes on macOS and the BSD
    /// family. On Windows it queries `GetProcessMemoryInfo` for
    /// `PeakWorkingSetSize`. If a platform does not expose peak measurements or
    /// the query fails, this value falls back to `0`.
    pub peak_memory_mb: usize,
    /// Number of files processed
    pub files_processed: usize,
    /// Total bytes extracted
    pub bytes_extracted: u64,
}

struct MemoryTracker {
    baseline_peak_bytes: Option<u64>,
}

impl MemoryTracker {
    fn start() -> Self {
        Self {
            baseline_peak_bytes: measure_peak_memory_bytes(),
        }
    }

    fn finish(&self) -> Option<usize> {
        let final_peak_bytes = measure_peak_memory_bytes();
        let bytes = match (self.baseline_peak_bytes, final_peak_bytes) {
            (Some(before), Some(after)) => Some(after.max(before)),
            (Some(before), None) => Some(before),
            (None, Some(after)) => Some(after),
            (None, None) => None,
        }?;

        Some(bytes_to_mebibytes(bytes))
    }
}

fn bytes_to_mebibytes(bytes: u64) -> usize {
    const MEBIBYTE: u64 = 1024 * 1024;
    ((bytes + MEBIBYTE - 1) / MEBIBYTE) as usize
}

fn measure_peak_memory_bytes() -> Option<u64> {
    #[cfg(test)]
    {
        if let Some(value) = {
            let mut guard = MOCK_PEAK_MEMORY_BYTES.lock().unwrap();
            guard.pop_front()
        } {
            return value;
        }
    }

    #[cfg(target_family = "unix")]
    {
        unix::peak_memory_bytes()
    }

    #[cfg(all(not(target_family = "unix"), target_os = "windows"))]
    {
        windows::peak_memory_bytes()
    }

    #[cfg(all(not(target_family = "unix"), not(target_os = "windows")))]
    {
        None
    }
}

#[cfg(target_family = "unix")]
mod unix {
    use libc::{getrusage, rusage, RUSAGE_SELF};

    pub(super) fn peak_memory_bytes() -> Option<u64> {
        unsafe {
            let mut usage = std::mem::MaybeUninit::<rusage>::uninit();
            if getrusage(RUSAGE_SELF, usage.as_mut_ptr()) == 0 {
                let usage = usage.assume_init();
                let maxrss = usage.ru_maxrss as u64;
                Some(maxrss_to_bytes(maxrss))
            } else {
                None
            }
        }
    }

    #[cfg(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly",
    ))]
    fn maxrss_to_bytes(value: u64) -> u64 {
        value
    }

    #[cfg(not(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly",
    )))]
    fn maxrss_to_bytes(value: u64) -> u64 {
        value * 1024
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use std::mem::{size_of, zeroed};
    use windows_sys::Win32::System::ProcessStatus::{
        GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    pub(super) fn peak_memory_bytes() -> Option<u64> {
        unsafe {
            let mut counters: PROCESS_MEMORY_COUNTERS = zeroed();
            counters.cb = size_of::<PROCESS_MEMORY_COUNTERS>() as u32;

            if GetProcessMemoryInfo(
                GetCurrentProcess(),
                &mut counters as *mut _ as *mut _,
                size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
            ) != 0
            {
                Some(counters.PeakWorkingSetSize as u64)
            } else {
                None
            }
        }
    }
}

#[cfg(test)]
static MOCK_PEAK_MEMORY_BYTES: Lazy<Mutex<VecDeque<Option<u64>>>> =
    Lazy::new(|| Mutex::new(VecDeque::new()));

#[cfg(test)]
pub(super) fn push_mock_peak_memory_bytes<I>(values: I)
where
    I: IntoIterator<Item = Option<u64>>,
{
    let mut guard = MOCK_PEAK_MEMORY_BYTES.lock().unwrap();
    guard.extend(values);
}

#[cfg(test)]
pub(super) fn reset_mock_peak_memory_bytes() {
    MOCK_PEAK_MEMORY_BYTES.lock().unwrap().clear();
}

/// Main extraction engine
pub struct Extractor {
    plugin_registry: Arc<PluginRegistry>,
    compliance_checker: Arc<ComplianceChecker>,
    config: Config,
    security_manager: Option<SecurityManager>,
}

impl Extractor {
    /// Create a new extractor with registries
    pub fn new(plugin_registry: PluginRegistry, compliance_registry: ComplianceRegistry) -> Self {
        let compliance_checker = ComplianceChecker::from_registry(compliance_registry);
        Self {
            plugin_registry: Arc::new(plugin_registry),
            compliance_checker: Arc::new(compliance_checker),
            config: Config::default(),
            security_manager: None,
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
            plugin_registry: Arc::new(plugin_registry),
            compliance_checker: Arc::new(compliance_checker),
            config,
            security_manager: None,
        }
    }

    /// Get a reference to the plugin registry
    pub fn plugin_registry(&self) -> &PluginRegistry {
        &self.plugin_registry
    }

    /// Clone extractor for safe worker sharing
    pub fn clone_for_worker(&self) -> Self {
        Self {
            plugin_registry: Arc::clone(&self.plugin_registry),
            compliance_checker: Arc::clone(&self.compliance_checker),
            config: self.config.clone(),
            security_manager: None, // Workers don't need security manager
        }
    }

    /// Get a reference to the compliance checker
    pub fn compliance_checker(&self) -> &ComplianceChecker {
        &self.compliance_checker
    }

    /// Initialize security manager (async operation)
    pub async fn init_security(&mut self) -> Result<(), ExtractionError> {
        match SecurityManager::new().await {
            Ok(security_manager) => {
                info!("Security framework initialized successfully");
                self.security_manager = Some(security_manager);
                Ok(())
            }
            Err(e) => {
                warn!("Failed to initialize security framework: {}", e);
                // Continue without security framework
                Ok(())
            }
        }
    }

    /// Check if security framework is enabled
    pub fn has_security(&self) -> bool {
        self.security_manager.as_ref().map_or(false, |sm| sm.is_enabled())
    }

    /// Extract assets from a file
    pub async fn extract_from_file(
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
        let (handler, plugin_info) = self
            .plugin_registry
            .find_handler_with_info(source_path, &header)
            .ok_or_else(|| ExtractionError::NoSuitablePlugin(source_path.to_path_buf()))?;

        info!("Using plugin handler: {} v{} for: {}", 
              plugin_info.name(), plugin_info.version(), source_path.display());

        // Security validation if enabled
        let mut security_report = None;
        if let Some(ref security_manager) = self.security_manager {
            info!("Running security validation...");
            match security_manager.validate_plugin(source_path).await {
                Ok(report) => {
                    if !report.plugin_approved {
                        return Err(ExtractionError::Generic(anyhow::anyhow!(
                            "Plugin security validation failed: threat level {:?}, score: {}",
                            report.threat_level, report.security_score
                        )));
                    }
                    security_report = Some(report);
                }
                Err(e) => {
                    warn!("Security validation failed: {}", e);
                    // Continue without security validation in case of framework issues
                }
            }
        }

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

        let memory_tracker = MemoryTracker::start();
        
        // SECURITY: Check initial memory usage before extraction
        let initial_memory_mb = memory_tracker.finish().unwrap_or(0);
        if initial_memory_mb > self.config.max_memory_mb {
            return Err(ExtractionError::MemoryLimitExceeded {
                limit: self.config.max_memory_mb,
                required: initial_memory_mb,
            });
        }

        // Extract resources using the handler
        let entries = handler.list_entries()?;

        info!("Found {} entries in archive", entries.len());

        // Extract actual resource data through the handler
        let mut resources = Vec::new();
        let mut total_extracted_bytes = 0u64;
        
        info!("Starting extraction of {} entries with memory limit: {}MB", entries.len(), self.config.max_memory_mb);
        
        for (index, entry) in entries.into_iter().enumerate() {
            let resource_type = match entry.file_type.as_deref() {
                Some("texture") => crate::resource::ResourceType::Texture,
                Some("mesh") => crate::resource::ResourceType::Mesh,
                Some("audio") => crate::resource::ResourceType::Audio,
                _ => crate::resource::ResourceType::Generic,
            };

            // SECURITY: Check memory usage before each entry extraction
            if index % 10 == 0 && index > 0 {
                let current_memory_mb = MemoryTracker::start().finish().unwrap_or(0);
                if current_memory_mb > self.config.max_memory_mb {
                    warn!("Memory limit exceeded during extraction at entry {}: {}MB > {}MB", 
                          index, current_memory_mb, self.config.max_memory_mb);
                    return Err(ExtractionError::MemoryLimitExceeded {
                        limit: self.config.max_memory_mb,
                        required: current_memory_mb,
                    });
                }
                info!("Memory check at entry {}: {}MB/{} MB", index, current_memory_mb, self.config.max_memory_mb);
            }
            
            // Actually read the entry data through the handler
            match handler.read_entry(&entry.id) {
                Ok(entry_data) => {
                    let actual_size = entry_data.len() as u64;
                    
                    // SECURITY: Check if this entry would exceed our memory budget
                    let estimated_memory_mb = (total_extracted_bytes + actual_size) / (1024 * 1024);
                    if estimated_memory_mb > self.config.max_memory_mb as u64 {
                        warn!("Single entry {} would exceed memory limit: {} MB", entry.name, estimated_memory_mb);
                        return Err(ExtractionError::MemoryLimitExceeded {
                            limit: self.config.max_memory_mb,
                            required: estimated_memory_mb as usize,
                        });
                    }
                    
                    total_extracted_bytes += actual_size;
                    
                    // Store the actual data in metadata for now
                    // In a real implementation, this would be processed further based on resource type
                    let mut metadata = std::collections::HashMap::new();
                    metadata.insert("data_size".to_string(), actual_size.to_string());
                    metadata.insert("extracted".to_string(), "true".to_string());
                    
                    if let Some(file_type) = &entry.file_type {
                        metadata.insert("original_format".to_string(), file_type.clone());
                    }

                    let resource = crate::resource::Resource {
                        id: entry.id.0.clone(),
                        name: entry.name.clone(),
                        resource_type,
                        size: actual_size,
                        format: entry.file_type.unwrap_or_else(|| "unknown".to_string()),
                        metadata,
                    };
                    resources.push(resource);
                    
                    info!("Successfully extracted entry: {} ({} bytes)", entry.name, actual_size);
                }
                Err(e) => {
                    warn!("Failed to extract entry {}: {}", entry.name, e);
                    // Create a resource entry to track the failure
                    let mut metadata = std::collections::HashMap::new();
                    metadata.insert("extraction_failed".to_string(), "true".to_string());
                    metadata.insert("error".to_string(), e.to_string());
                    
                    let resource = crate::resource::Resource {
                        id: entry.id.0.clone(),
                        name: entry.name.clone(),
                        resource_type,
                        size: 0,
                        format: entry.file_type.unwrap_or_else(|| "unknown".to_string()),
                        metadata,
                    };
                    resources.push(resource);
                }
            }
        }

        // Security scan of extracted assets if enabled
        if let Some(ref security_manager) = self.security_manager {
            if let Some(ref mut report) = security_report {
                info!("Running security scan on extracted assets...");
                match security_manager.scan_extracted_assets(_output_dir).await {
                    Ok(asset_scan_report) => {
                        // Merge asset scan results with plugin validation
                        report.warnings.extend(asset_scan_report.warnings);
                        // Use the lower of the two security scores
                        if asset_scan_report.security_score < report.security_score {
                            report.security_score = asset_scan_report.security_score;
                            report.threat_level = asset_scan_report.threat_level;
                            report.compliance_status = asset_scan_report.compliance_status;
                        }
                    }
                    Err(e) => {
                        warn!("Asset security scan failed: {}", e);
                    }
                }
            }
        }

        let duration = start_time.elapsed();
        let peak_memory_mb = match memory_tracker.finish() {
            Some(value) => value,
            None => {
                warn!("Peak memory usage measurement is not available on this platform");
                0
            }
        };
        let metrics = ExtractionMetrics {
            duration_ms: duration.as_millis() as u64,
            peak_memory_mb,
            files_processed: resources.len(),
            bytes_extracted: total_extracted_bytes,
        };

        let compliance_info = ComplianceInfo {
            is_compliant: is_allowed,
            risk_level,
            warnings,
            recommendations,
        };

        info!(
            "Extraction completed in {}ms, {} resources extracted (peak memory: {} MB)",
            metrics.duration_ms,
            resources.len(),
            metrics.peak_memory_mb
        );

        Ok(ExtractionResult {
            source_path: source_path.to_path_buf(),
            resources,
            warnings: vec![],
            compliance_info,
            security_report,
            metrics,
        })
    }

    /// Batch extract multiple files
    pub async fn extract_batch(
        &mut self,
        sources: Vec<&Path>,
        output_dir: &Path,
    ) -> Result<Vec<ExtractionResult>, ExtractionError> {
        let mut results = Vec::new();

        for source in sources {
            match self.extract_from_file(source, output_dir).await {
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

// Extractor is now automatically Send + Sync because Arc<T> is Send + Sync when T: Send + Sync
// REMOVED UNSAFE MANUAL IMPLEMENTATIONS - Arc handles this safely

#[cfg(test)]
mod tests {
    use super::{
        push_mock_peak_memory_bytes, reset_mock_peak_memory_bytes, ExtractionError, Extractor,
    };
    use anyhow::Result;
    use std::collections::HashMap;
    use std::fs::File;
    use std::io::Write;
    use std::path::Path;
    use tempfile::TempDir;

    use crate::{
        archive::{
            ArchiveHandler, ComplianceLevel, ComplianceProfile, EntryId, EntryMetadata, PluginInfo,
            Provenance,
        },
        ComplianceRegistry, PluginFactory,
    };
    use chrono::Utc;
    use uuid::Uuid;

    #[test]
    fn test_extractor_creation() {
        let plugin_registry = crate::PluginRegistry::new();
        let compliance = ComplianceRegistry::new();
        let extractor = Extractor::new(plugin_registry, compliance);
        let stats = extractor.get_stats();

        assert_eq!(stats.files_processed, 0);
    }

    #[tokio::test]
    async fn test_file_not_found() {
        let plugin_registry = crate::PluginRegistry::new();
        let compliance = ComplianceRegistry::new();
        let mut extractor = Extractor::new(plugin_registry, compliance);
        let temp_dir = TempDir::new().unwrap();

        let result =
            extractor.extract_from_file(&temp_dir.path().join("nonexistent.file"), temp_dir.path()).await;

        assert!(matches!(result, Err(ExtractionError::FileNotFound(_))));
    }

    #[tokio::test]
    async fn test_mock_extraction() {
        let plugin_registry = crate::PluginRegistry::new();
        let compliance = ComplianceRegistry::new();
        let mut extractor = Extractor::new(plugin_registry, compliance);
        let temp_dir = TempDir::new().unwrap();

        // Create a dummy file
        let test_file = temp_dir.path().join("test.dat");
        let mut file = File::create(&test_file).unwrap();
        file.write_all(b"test data").unwrap();

        let result = extractor.extract_from_file(&test_file, temp_dir.path()).await;
        // This will likely fail since we have no plugins registered, but that's expected
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_peak_memory_metrics_are_recorded() {
        reset_mock_peak_memory_bytes();
        let baseline = 100 * 1024 * 1024;
        let final_peak = 150 * 1024 * 1024;
        push_mock_peak_memory_bytes([Some(baseline), Some(final_peak)]);

        let mut extractor = create_mock_extractor();
        let temp_dir = TempDir::new().unwrap();
        let test_file = create_test_archive(&temp_dir, "sample.mock");

        let result = extractor
            .extract_from_file(&test_file, temp_dir.path())
            .await
            .expect("extraction should succeed");

        assert_eq!(result.metrics.peak_memory_mb, 150);
        assert_eq!(result.metrics.files_processed, 1);
    }

    #[tokio::test]
    async fn test_peak_memory_metrics_default_to_zero_when_unavailable() {
        reset_mock_peak_memory_bytes();
        push_mock_peak_memory_bytes([None, None]);

        let mut extractor = create_mock_extractor();
        let temp_dir = TempDir::new().unwrap();
        let test_file = create_test_archive(&temp_dir, "fallback.mock");

        let result = extractor
            .extract_from_file(&test_file, temp_dir.path())
            .await
            .expect("extraction should succeed");

        assert_eq!(result.metrics.peak_memory_mb, 0);
    }

    fn create_mock_extractor() -> Extractor {
        let mut plugin_registry = crate::PluginRegistry::new();
        plugin_registry.register_plugin(Box::new(MockPluginFactory));
        let compliance = ComplianceRegistry::new();
        Extractor::new(plugin_registry, compliance)
    }

    fn create_test_archive(temp_dir: &TempDir, name: &str) -> std::path::PathBuf {
        let path = temp_dir.path().join(name);
        let mut file = File::create(&path).unwrap();
        file.write_all(b"mock archive contents").unwrap();
        path
    }

    struct MockPluginFactory;

    impl PluginFactory for MockPluginFactory {
        fn name(&self) -> &str {
            "mock-plugin"
        }

        fn version(&self) -> &str {
            "1.0"
        }

        fn supported_extensions(&self) -> Vec<&str> {
            vec!["mock"]
        }

        fn can_handle(&self, _bytes: &[u8]) -> bool {
            true
        }

        fn create_handler(&self, path: &Path) -> Result<Box<dyn ArchiveHandler>> {
            Ok(Box::new(MockArchiveHandler::open(path)?))
        }

        fn compliance_info(&self) -> crate::PluginInfo {
            crate::PluginInfo::new(
                "mock-plugin".to_string(),
                "1.0".to_string(),
                vec!["mock".to_string()],
            )
        }
    }

    struct MockArchiveHandler {
        entries: Vec<EntryMetadata>,
        profile: ComplianceProfile,
        provenance: Provenance,
    }

    impl ArchiveHandler for MockArchiveHandler {
        fn detect(_bytes: &[u8]) -> bool
        where
            Self: Sized,
        {
            true
        }

        fn open(path: &Path) -> Result<Self>
        where
            Self: Sized,
        {
            let profile = ComplianceProfile {
                publisher: "Test Publisher".to_string(),
                game_id: Some("test_game".to_string()),
                enforcement_level: ComplianceLevel::Permissive,
                official_support: true,
                bounty_eligible: false,
                enterprise_warning: None,
                mod_policy_url: None,
                supported_formats: HashMap::new(),
            };

            let provenance = Provenance {
                session_id: Uuid::new_v4(),
                game_id: Some("test_game".to_string()),
                source_hash: "dummy".to_string(),
                source_path: path.to_path_buf(),
                compliance_profile: profile.clone(),
                extraction_time: Utc::now(),
                aegis_version: "test".to_string(),
                plugin_info: PluginInfo {
                    name: "mock-plugin".to_string(),
                    version: "1.0".to_string(),
                    author: Some("Test".to_string()),
                    compliance_verified: true,
                },
            };

            let entries = vec![EntryMetadata {
                id: EntryId::new("entry-1"),
                name: "entry.bin".to_string(),
                path: path.to_path_buf(),
                size_compressed: None,
                size_uncompressed: 1024,
                file_type: Some("generic".to_string()),
                last_modified: None,
                checksum: None,
            }];

            Ok(Self {
                entries,
                profile,
                provenance,
            })
        }

        fn compliance_profile(&self) -> &ComplianceProfile {
            &self.profile
        }

        fn list_entries(&self) -> Result<Vec<EntryMetadata>> {
            Ok(self.entries.clone())
        }

        fn read_entry(&self, _id: &EntryId) -> Result<Vec<u8>> {
            Ok(vec![])
        }

        fn provenance(&self) -> &Provenance {
            &self.provenance
        }
    }
}
