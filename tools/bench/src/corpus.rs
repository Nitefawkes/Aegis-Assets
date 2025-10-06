use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Test corpus loader and validator
pub struct CorpusLoader {
    corpus_path: PathBuf,
    manifest: Option<CorpusManifest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusManifest {
    pub version: String,
    pub created: String,
    pub purpose: String,
    pub samples: Vec<SampleSpec>,
    pub performance_targets: PerformanceTargets,
    pub validation: ValidationConfig,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SampleSpec {
    pub name: String,
    pub description: String,
    pub target_size: String,
    pub compression: String,
    pub primary_assets: Vec<String>,
    pub file_pattern: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceTargets {
    pub memory_limit_mb: usize,
    pub throughput_min_mbps: f64,
    pub processing_time_max_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationConfig {
    pub texture_formats: Vec<String>,
    pub mesh_formats: Vec<String>,
    pub audio_formats: Vec<String>,
    pub metadata_required: bool,
    pub golden_checksums: bool,
}

impl CorpusLoader {
    pub fn new(corpus_path: PathBuf) -> Result<Self> {
        let mut loader = Self {
            corpus_path,
            manifest: None,
        };
        
        loader.load_manifest()?;
        Ok(loader)
    }

    fn load_manifest(&mut self) -> Result<()> {
        let manifest_path = self.corpus_path.join("manifest.yaml");
        
        if manifest_path.exists() {
            let content = std::fs::read_to_string(&manifest_path)
                .context("Failed to read manifest file")?;
            
            let manifest: CorpusManifest = serde_yaml::from_str(&content)
                .context("Failed to parse manifest YAML")?;
            
            self.manifest = Some(manifest);
        }
        
        Ok(())
    }

    pub async fn load_test_files(&self) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        
        // If we have a manifest, use it to find files
        if let Some(ref manifest) = self.manifest {
            for sample in &manifest.samples {
                if sample.status == "available" || sample.status == "ready" {
                    let pattern_files = self.find_files_by_pattern(&sample.file_pattern)?;
                    files.extend(pattern_files);
                }
            }
        } else {
            // Fall back to finding all Unity files
            files = self.find_unity_files()?;
        }

        // Filter and validate files
        let mut valid_files = Vec::new();
        for file_path in files {
            if self.is_valid_test_file(&file_path).await? {
                valid_files.push(file_path);
            }
        }

        // Sort by file size for predictable benchmarking order
        valid_files.sort_by_key(|path| {
            std::fs::metadata(path)
                .map(|m| m.len())
                .unwrap_or(0)
        });

        Ok(valid_files)
    }

    fn find_files_by_pattern(&self, pattern: &str) -> Result<Vec<PathBuf>> {
        let glob_pattern = self.corpus_path.join(pattern);
        let pattern_str = glob_pattern.to_string_lossy();
        
        let mut files = Vec::new();
        for entry in glob::glob(&pattern_str)? {
            match entry {
                Ok(path) => files.push(path),
                Err(e) => tracing::warn!("Glob error: {}", e),
            }
        }
        
        Ok(files)
    }

    fn find_unity_files(&self) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        
        for entry in WalkDir::new(&self.corpus_path) {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_file() {
                if let Some(extension) = path.extension() {
                    let ext_str = extension.to_string_lossy().to_lowercase();
                    if ext_str == "unity3d" || ext_str == "assets" || ext_str == "bundle" {
                        files.push(path.to_path_buf());
                    }
                }
            }
        }
        
        Ok(files)
    }

    async fn is_valid_test_file(&self, file_path: &Path) -> Result<bool> {
        // Check if file exists and is readable
        if !file_path.exists() || !file_path.is_file() {
            return Ok(false);
        }

        // Check file size (should be > 1KB, < 1GB for reasonable testing)
        let metadata = std::fs::metadata(file_path)?;
        let size_bytes = metadata.len();
        
        if size_bytes < 1024 || size_bytes > 1024 * 1024 * 1024 {
            tracing::warn!("File size out of range: {} bytes ({})", size_bytes, file_path.display());
            return Ok(false);
        }

        // Check if we can read the file header
        let mut file = std::fs::File::open(file_path)?;
        let mut header = vec![0u8; 256];
        use std::io::Read;
        let bytes_read = file.read(&mut header)?;
        
        if bytes_read < 16 {
            tracing::warn!("File too small or unreadable: {}", file_path.display());
            return Ok(false);
        }

        // Basic Unity file format detection
        if self.is_unity_file(&header) {
            return Ok(true);
        }

        // Could add other format detection here

        Ok(false)
    }

    fn is_unity_file(&self, header: &[u8]) -> bool {
        // Check for Unity signatures
        if header.len() >= 8 {
            // UnityFS signature
            if header.starts_with(b"UnityFS\0") {
                return true;
            }
            
            // Unity serialized file signature
            if header.len() >= 20 {
                // Check for metadata size and file size patterns typical of Unity files
                let metadata_size = u32::from_be_bytes([header[8], header[9], header[10], header[11]]);
                if metadata_size > 0 && metadata_size < 1024 * 1024 {
                    return true;
                }
            }
        }

        false
    }

    pub async fn validate(&self) -> Result<CorpusValidationResult> {
        let mut result = CorpusValidationResult {
            total_files_found: 0,
            valid_files: 0,
            invalid_files: Vec::new(),
            missing_samples: Vec::new(),
            size_distribution: SizeDistribution::default(),
            format_distribution: FormatDistribution::default(),
            issues: Vec::new(),
        };

        // Load all files
        let files = self.load_test_files().await?;
        result.total_files_found = files.len();

        if files.is_empty() {
            result.issues.push("No valid test files found in corpus".to_string());
            return Ok(result);
        }

        // Validate each file
        for file_path in &files {
            if self.is_valid_test_file(file_path).await? {
                result.valid_files += 1;
                
                // Update size distribution
                if let Ok(metadata) = std::fs::metadata(file_path) {
                    let size_mb = metadata.len() as f64 / (1024.0 * 1024.0);
                    result.size_distribution.update(size_mb);
                }

                // Update format distribution
                if let Some(ext) = file_path.extension() {
                    let ext_str = ext.to_string_lossy().to_lowercase();
                    result.format_distribution.update(&ext_str);
                }
            } else {
                result.invalid_files.push(file_path.to_string_lossy().to_string());
            }
        }

        // Check manifest compliance
        if let Some(ref manifest) = self.manifest {
            for sample in &manifest.samples {
                if sample.status == "needed" {
                    result.missing_samples.push(sample.name.clone());
                }
            }
        }

        // Quality checks
        if result.valid_files < 3 {
            result.issues.push("Insufficient test files for reliable benchmarking".to_string());
        }

        if result.size_distribution.total_size_gb > 2.0 {
            result.issues.push("Corpus size may be too large for CI environments".to_string());
        }

        if result.missing_samples.len() > result.valid_files {
            result.issues.push("More samples needed than available".to_string());
        }

        Ok(result)
    }

    pub fn get_performance_targets(&self) -> Option<&PerformanceTargets> {
        self.manifest.as_ref().map(|m| &m.performance_targets)
    }

    pub fn get_validation_config(&self) -> Option<&ValidationConfig> {
        self.manifest.as_ref().map(|m| &m.validation)
    }
}

#[derive(Debug)]
pub struct CorpusValidationResult {
    pub total_files_found: usize,
    pub valid_files: usize,
    pub invalid_files: Vec<String>,
    pub missing_samples: Vec<String>,
    pub size_distribution: SizeDistribution,
    pub format_distribution: FormatDistribution,
    pub issues: Vec<String>,
}

#[derive(Debug, Default)]
pub struct SizeDistribution {
    pub total_size_gb: f64,
    pub min_size_mb: f64,
    pub max_size_mb: f64,
    pub avg_size_mb: f64,
    pub count: usize,
}

impl SizeDistribution {
    fn update(&mut self, size_mb: f64) {
        if self.count == 0 {
            self.min_size_mb = size_mb;
            self.max_size_mb = size_mb;
            self.avg_size_mb = size_mb;
        } else {
            self.min_size_mb = self.min_size_mb.min(size_mb);
            self.max_size_mb = self.max_size_mb.max(size_mb);
            self.avg_size_mb = (self.avg_size_mb * self.count as f64 + size_mb) / (self.count + 1) as f64;
        }
        
        self.count += 1;
        self.total_size_gb += size_mb / 1024.0;
    }
}

#[derive(Debug, Default)]
pub struct FormatDistribution {
    pub formats: std::collections::HashMap<String, usize>,
}

impl FormatDistribution {
    fn update(&mut self, format: &str) {
        *self.formats.entry(format.to_string()).or_insert(0) += 1;
    }
}
