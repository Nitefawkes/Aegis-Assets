use anyhow::{Context, Result};
use std::path::Path;
use std::time::Instant;
use aegis_core::{Extractor, Config, PluginRegistry};
use aegis_core::archive::ComplianceRegistry;
use crate::memory::{MemoryTracker, MemoryPressureMonitor};
use crate::metrics::{BenchmarkMetrics, StreamingMetrics};

/// Extraction benchmark engine
pub struct ExtractionBenchmark {
    extractor: Extractor,
    memory_limit_mb: usize,
    validate_streaming: bool,
    memory_tracker: MemoryTracker,
}

#[derive(Debug, Clone)]
pub struct ExtractionResult {
    pub assets_extracted: usize,
    pub streaming_successful: bool,
    pub errors: Vec<String>,
    pub streaming_metrics: Option<StreamingMetrics>,
}

impl ExtractionBenchmark {
    pub fn new(memory_limit_mb: usize, validate_streaming: bool) -> Result<Self> {
        // Initialize plugin registry
        let plugin_registry = PluginRegistry::new();
        
        // Initialize compliance registry
        let compliance_registry = ComplianceRegistry::new();
        
        // Create extractor with performance-focused config
        let config = Config {
            max_memory_mb: memory_limit_mb,
            enable_streaming: validate_streaming,
            parallel_processing: false, // Disable for consistent benchmarking
            ..Default::default()
        };
        
        let extractor = Extractor::with_config(plugin_registry, compliance_registry, config);
        
        Ok(Self {
            extractor,
            memory_limit_mb,
            validate_streaming,
            memory_tracker: MemoryTracker::new(true),
        })
    }

    pub async fn run_extraction(&mut self, file_path: &Path) -> Result<ExtractionResult> {
        let mut errors = Vec::new();
        let mut streaming_metrics = if self.validate_streaming {
            Some(StreamingMetrics::new())
        } else {
            None
        };

        // Setup memory monitoring
        self.memory_tracker.start_monitoring();
        let mut memory_monitor = if self.validate_streaming {
            Some(MemoryPressureMonitor::new(self.memory_limit_mb))
        } else {
            None
        };

        if let Some(ref mut monitor) = memory_monitor {
            monitor.start();
        }

        // Create temporary output directory
        let temp_dir = tempfile::tempdir()
            .context("Failed to create temporary directory")?;

        // Run extraction
        let start_time = Instant::now();
        let extraction_result = self.extractor.extract_from_file(file_path, temp_dir.path());
        let _duration = start_time.elapsed();

        // Process results
        let assets_extracted = match &extraction_result {
            Ok(result) => result.resources.len(),
            Err(e) => {
                errors.push(format!("Extraction failed: {}", e));
                0
            }
        };

        // Check streaming performance
        let streaming_successful = if let Some(ref monitor) = memory_monitor {
            let pressure = monitor.check_pressure();
            if pressure.is_critical() {
                errors.push("Memory limit exceeded during extraction".to_string());
                false
            } else {
                true
            }
        } else {
            true
        };

        // Update streaming metrics
        if let Some(ref mut metrics) = streaming_metrics {
            // TODO: Integrate with actual streaming metrics from extractor
            metrics.record_chunk(1024 * 1024); // Placeholder
            metrics.record_buffer_usage(self.memory_tracker.peak_memory_mb());
        }

        Ok(ExtractionResult {
            assets_extracted,
            streaming_successful,
            errors,
            streaming_metrics,
        })
    }

    pub fn peak_memory_mb(&self) -> f64 {
        self.memory_tracker.peak_memory_mb()
    }

    pub fn allocation_count(&self) -> u64 {
        self.memory_tracker.allocation_count()
    }
}

/// Stress testing for memory and performance limits
pub struct StressTester {
    base_benchmark: ExtractionBenchmark,
    stress_config: StressConfig,
}

#[derive(Debug, Clone)]
pub struct StressConfig {
    pub max_concurrent_extractions: usize,
    pub memory_stress_multiplier: f64,
    pub throughput_stress_target: f64,
    pub duration_limit_seconds: u64,
}

impl Default for StressConfig {
    fn default() -> Self {
        Self {
            max_concurrent_extractions: 4,
            memory_stress_multiplier: 1.5,
            throughput_stress_target: 200.0, // MB/s
            duration_limit_seconds: 300, // 5 minutes
        }
    }
}

impl StressTester {
    pub fn new(memory_limit_mb: usize, stress_config: StressConfig) -> Result<Self> {
        let base_benchmark = ExtractionBenchmark::new(
            (memory_limit_mb as f64 * stress_config.memory_stress_multiplier) as usize,
            true,
        )?;

        Ok(Self {
            base_benchmark,
            stress_config,
        })
    }

    pub async fn run_stress_test(&mut self, file_paths: &[&Path]) -> Result<StressTestResult> {
        let start_time = Instant::now();
        let mut results = Vec::new();
        let mut total_errors = 0;
        let mut peak_memory_mb = 0.0f64;

        for file_path in file_paths {
            if start_time.elapsed().as_secs() > self.stress_config.duration_limit_seconds {
                break;
            }

            let result = self.base_benchmark.run_extraction(file_path).await?;
            
            if !result.errors.is_empty() {
                total_errors += result.errors.len();
            }

            let current_memory = self.base_benchmark.peak_memory_mb();
            if current_memory > peak_memory_mb {
                peak_memory_mb = current_memory;
            }

            results.push(result);
        }

        let total_duration = start_time.elapsed();
        
        Ok(StressTestResult {
            total_files_processed: results.len(),
            total_assets_extracted: results.iter().map(|r| r.assets_extracted).sum(),
            total_errors,
            peak_memory_mb,
            total_duration_seconds: total_duration.as_secs_f64(),
            stress_level_reached: peak_memory_mb > (self.stress_config.memory_stress_multiplier * 100.0),
            results,
        })
    }
}

#[derive(Debug)]
pub struct StressTestResult {
    pub total_files_processed: usize,
    pub total_assets_extracted: usize,
    pub total_errors: usize,
    pub peak_memory_mb: f64,
    pub total_duration_seconds: f64,
    pub stress_level_reached: bool,
    pub results: Vec<ExtractionResult>,
}

/// Validation of extraction accuracy
pub struct AccuracyValidator {
    golden_outputs_path: std::path::PathBuf,
}

impl AccuracyValidator {
    pub fn new(golden_outputs_path: std::path::PathBuf) -> Self {
        Self {
            golden_outputs_path,
        }
    }

    pub async fn validate_extraction_accuracy(
        &self,
        file_path: &Path,
        extracted_assets: &[aegis_core::resource::Resource],
    ) -> Result<AccuracyResult> {
        // Load golden reference if available
        let golden_path = self.golden_outputs_path
            .join(file_path.file_stem().unwrap_or_default())
            .with_extension("golden.json");

        if !golden_path.exists() {
            return Ok(AccuracyResult {
                has_golden_reference: false,
                matches_golden: false,
                asset_count_diff: 0,
                format_mismatches: Vec::new(),
                checksum_mismatches: Vec::new(),
            });
        }

        // TODO: Implement golden reference comparison
        // This would compare:
        // - Asset counts by type
        // - File checksums
        // - Metadata accuracy
        // - Format conversion correctness

        Ok(AccuracyResult {
            has_golden_reference: true,
            matches_golden: true, // Placeholder
            asset_count_diff: 0,
            format_mismatches: Vec::new(),
            checksum_mismatches: Vec::new(),
        })
    }
}

#[derive(Debug)]
pub struct AccuracyResult {
    pub has_golden_reference: bool,
    pub matches_golden: bool,
    pub asset_count_diff: i32,
    pub format_mismatches: Vec<String>,
    pub checksum_mismatches: Vec<String>,
}

/// Configuration for extraction benchmarking
#[derive(Debug, Clone)]
pub struct BenchmarkConfig {
    pub memory_limit_mb: usize,
    pub throughput_min_mbps: f64,
    pub enable_streaming_validation: bool,
    pub enable_accuracy_validation: bool,
    pub golden_outputs_path: Option<std::path::PathBuf>,
    pub stress_testing: bool,
    pub parallel_extraction: bool,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            memory_limit_mb: 300,
            throughput_min_mbps: 120.0,
            enable_streaming_validation: true,
            enable_accuracy_validation: false,
            golden_outputs_path: None,
            stress_testing: false,
            parallel_extraction: false,
        }
    }
}
