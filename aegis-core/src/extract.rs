use crate::{
    archive::ComplianceRegistry,
    compliance::{ComplianceChecker, ComplianceResult},
    resource::Resource,
    Config, PluginRegistry,
};
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
        let plugin_factory = self
            .plugin_registry
            .find_handler(source_path, &header)
            .ok_or_else(|| ExtractionError::NoSuitablePlugin(source_path.to_path_buf()))?;

        info!(
            "Using plugin: {} v{}",
            plugin_factory.name(),
            plugin_factory.version()
        );

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
            bytes_extracted: total_bytes,
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
    fn test_peak_memory_metrics_are_recorded() {
        reset_mock_peak_memory_bytes();
        let baseline = 100 * 1024 * 1024;
        let final_peak = 150 * 1024 * 1024;
        push_mock_peak_memory_bytes([Some(baseline), Some(final_peak)]);

        let mut extractor = create_mock_extractor();
        let temp_dir = TempDir::new().unwrap();
        let test_file = create_test_archive(&temp_dir, "sample.mock");

        let result = extractor
            .extract_from_file(&test_file, temp_dir.path())
            .expect("extraction should succeed");

        assert_eq!(result.metrics.peak_memory_mb, 150);
        assert_eq!(result.metrics.files_processed, 1);
    }

    #[test]
    fn test_peak_memory_metrics_default_to_zero_when_unavailable() {
        reset_mock_peak_memory_bytes();
        push_mock_peak_memory_bytes([None, None]);

        let mut extractor = create_mock_extractor();
        let temp_dir = TempDir::new().unwrap();
        let test_file = create_test_archive(&temp_dir, "fallback.mock");

        let result = extractor
            .extract_from_file(&test_file, temp_dir.path())
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

        fn compliance_info(&self) -> PluginInfo {
            PluginInfo {
                name: "mock-plugin".to_string(),
                version: "1.0".to_string(),
                author: Some("Test".to_string()),
                compliance_verified: true,
            }
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
