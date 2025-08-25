use crate::archive::Provenance;
use anyhow::{Result, Context, bail};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use blake3::Hasher;

/// Patch recipe for recreating extracted assets without distributing copyrighted content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchRecipe {
    /// Version of the recipe format
    pub version: String,
    /// BLAKE3 hash of the source file
    pub source_hash: String,
    /// Size of the source file
    pub source_size: u64,
    /// Provenance information
    pub provenance: Provenance,
    /// Delta patches for recreating assets
    pub deltas: Vec<DeltaPatch>,
    /// Metadata about the extracted assets
    pub asset_metadata: Vec<AssetMetadata>,
    /// Recipe creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Binary delta patch for recreating an asset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeltaPatch {
    /// Target output file name
    pub target_file: String,
    /// Offset in source file to start reading
    pub source_offset: u64,
    /// Length to read from source
    pub source_length: u64,
    /// Transformation operations to apply
    pub operations: Vec<PatchOperation>,
    /// Expected output hash (BLAKE3)
    pub output_hash: String,
    /// Expected output size
    pub output_size: u64,
}

/// Operations that can be applied to transform source data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PatchOperation {
    /// Copy bytes as-is
    Copy { offset: u64, length: u64 },
    /// Decompress using specified algorithm
    Decompress { algorithm: CompressionAlgorithm, expected_size: u64 },
    /// Apply format conversion
    Convert { from_format: String, to_format: String, parameters: HashMap<String, serde_json::Value> },
    /// Insert header bytes
    InsertHeader { data: Vec<u8> },
    /// Append footer bytes
    AppendFooter { data: Vec<u8> },
    /// XOR with key (simple obfuscation reversal)
    Xor { key: Vec<u8> },
}

/// Supported compression algorithms
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompressionAlgorithm {
    Zlib,
    Deflate,
    Lz4,
    Lzma,
    Gzip,
    Custom(String),
}

/// Metadata about an extracted asset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetMetadata {
    /// Asset file name
    pub name: String,
    /// Asset type (texture, mesh, audio, etc.)
    pub asset_type: String,
    /// Estimated extraction time (milliseconds)
    pub extraction_time_ms: u64,
    /// Asset properties for validation
    pub properties: HashMap<String, serde_json::Value>,
}

/// Builder for creating patch recipes
pub struct PatchRecipeBuilder {
    source_path: PathBuf,
    source_hash: Option<String>,
    provenance: Option<Provenance>,
    deltas: Vec<DeltaPatch>,
    asset_metadata: Vec<AssetMetadata>,
}

impl PatchRecipeBuilder {
    /// Create a new recipe builder
    pub fn new(source_path: impl Into<PathBuf>) -> Self {
        Self {
            source_path: source_path.into(),
            source_hash: None,
            provenance: None,
            deltas: Vec::new(),
            asset_metadata: Vec::new(),
        }
    }
    
    /// Set provenance information
    pub fn with_provenance(mut self, provenance: Provenance) -> Self {
        self.provenance = Some(provenance);
        self
    }
    
    /// Add a delta patch
    pub fn add_delta(mut self, delta: DeltaPatch) -> Self {
        self.deltas.push(delta);
        self
    }
    
    /// Add asset metadata
    pub fn add_asset_metadata(mut self, metadata: AssetMetadata) -> Self {
        self.asset_metadata.push(metadata);
        self
    }
    
    /// Build the patch recipe
    pub fn build(mut self) -> Result<PatchRecipe> {
        // Calculate source file hash if not provided
        if self.source_hash.is_none() {
            self.source_hash = Some(self.calculate_source_hash()?);
        }
        
        let source_size = std::fs::metadata(&self.source_path)
            .context("Failed to get source file metadata")?
            .len();
        
        let provenance = self.provenance
            .ok_or_else(|| anyhow::anyhow!("Provenance information is required"))?;
        
        Ok(PatchRecipe {
            version: "1.0".to_string(),
            source_hash: self.source_hash.unwrap(),
            source_size,
            provenance,
            deltas: self.deltas,
            asset_metadata: self.asset_metadata,
            created_at: chrono::Utc::now(),
        })
    }
    
    /// Calculate BLAKE3 hash of source file
    fn calculate_source_hash(&self) -> Result<String> {
        let mut hasher = Hasher::new();
        let mut file = std::fs::File::open(&self.source_path)
            .context("Failed to open source file for hashing")?;
        
        std::io::copy(&mut file, &mut hasher)
            .context("Failed to read source file for hashing")?;
        
        Ok(hasher.finalize().to_hex().to_string())
    }
}

/// Patch recipe applier for reconstructing assets
pub struct PatchApplier {
    recipe: PatchRecipe,
}

impl PatchApplier {
    /// Create a new patch applier
    pub fn new(recipe: PatchRecipe) -> Self {
        Self { recipe }
    }
    
    /// Load recipe from file
    pub fn from_file(recipe_path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(recipe_path)
            .context("Failed to read recipe file")?;
        
        let recipe: PatchRecipe = serde_json::from_str(&content)
            .context("Failed to parse recipe JSON")?;
        
        Ok(Self::new(recipe))
    }
    
    /// Apply the recipe to recreate assets
    pub fn apply(&self, source_file: &Path, output_dir: &Path) -> Result<ApplyResult> {
        let start_time = std::time::Instant::now();
        
        // Verify source file
        self.verify_source_file(source_file)?;
        
        // Create output directory
        std::fs::create_dir_all(output_dir)
            .context("Failed to create output directory")?;
        
        let mut applied_files = Vec::new();
        let mut warnings = Vec::new();
        
        // Apply each delta patch
        for delta in &self.recipe.deltas {
            match self.apply_delta_patch(delta, source_file, output_dir) {
                Ok(output_path) => {
                    applied_files.push(AppliedFile {
                        path: output_path.clone(),
                        size_bytes: std::fs::metadata(&output_path)?.len(),
                        source_delta: delta.target_file.clone(),
                    });
                }
                Err(e) => {
                    warnings.push(format!(
                        "Failed to apply delta for '{}': {}",
                        delta.target_file, e
                    ));
                }
            }
        }
        
        let duration = start_time.elapsed();
        
        Ok(ApplyResult {
            applied_files,
            warnings,
            duration_ms: duration.as_millis() as u64,
            recipe_version: self.recipe.version.clone(),
        })
    }
    
    /// Verify that the source file matches the recipe
    fn verify_source_file(&self, source_file: &Path) -> Result<()> {
        if !source_file.exists() {
            bail!("Source file does not exist: {}", source_file.display());
        }
        
        let metadata = std::fs::metadata(source_file)
            .context("Failed to get source file metadata")?;
        
        if metadata.len() != self.recipe.source_size {
            bail!(
                "Source file size mismatch: expected {}, got {}",
                self.recipe.source_size,
                metadata.len()
            );
        }
        
        // Calculate and verify hash
        let mut hasher = Hasher::new();
        let mut file = std::fs::File::open(source_file)
            .context("Failed to open source file for verification")?;
        
        std::io::copy(&mut file, &mut hasher)
            .context("Failed to read source file for verification")?;
        
        let calculated_hash = hasher.finalize().to_hex().to_string();
        
        if calculated_hash != self.recipe.source_hash {
            bail!(
                "Source file hash mismatch: expected {}, got {}",
                self.recipe.source_hash,
                calculated_hash
            );
        }
        
        Ok(())
    }
    
    /// Apply a single delta patch
    fn apply_delta_patch(
        &self,
        delta: &DeltaPatch,
        source_file: &Path,
        output_dir: &Path,
    ) -> Result<PathBuf> {
        use std::io::{Read, Seek, SeekFrom};
        
        let mut source = std::fs::File::open(source_file)
            .context("Failed to open source file")?;
        
        // Seek to the required offset
        source.seek(SeekFrom::Start(delta.source_offset))
            .context("Failed to seek in source file")?;
        
        // Read the required data
        let mut buffer = vec![0u8; delta.source_length as usize];
        source.read_exact(&mut buffer)
            .context("Failed to read from source file")?;
        
        // Apply transformation operations
        for operation in &delta.operations {
            buffer = self.apply_operation(operation, buffer)?;
        }
        
        // Verify output hash
        let output_hash = blake3::hash(&buffer).to_hex().to_string();
        if output_hash != delta.output_hash {
            bail!(
                "Output hash mismatch for {}: expected {}, got {}",
                delta.target_file,
                delta.output_hash,
                output_hash
            );
        }
        
        // Write output file
        let output_path = output_dir.join(&delta.target_file);
        std::fs::write(&output_path, &buffer)
            .context("Failed to write output file")?;
        
        Ok(output_path)
    }
    
    /// Apply a single transformation operation
    fn apply_operation(&self, operation: &PatchOperation, mut data: Vec<u8>) -> Result<Vec<u8>> {
        match operation {
            PatchOperation::Copy { offset, length } => {
                let start = *offset as usize;
                let end = start + (*length as usize);
                if end > data.len() {
                    bail!("Copy operation exceeds data bounds");
                }
                Ok(data[start..end].to_vec())
            }
            
            PatchOperation::Decompress { algorithm, expected_size } => {
                match algorithm {
                    CompressionAlgorithm::Zlib => {
                        use flate2::read::ZlibDecoder;
                        use std::io::Read;
                        
                        let mut decoder = ZlibDecoder::new(&data[..]);
                        let mut decompressed = Vec::new();
                        decoder.read_to_end(&mut decompressed)
                            .context("Zlib decompression failed")?;
                        
                        if decompressed.len() != *expected_size as usize {
                            bail!("Decompressed size mismatch");
                        }
                        
                        Ok(decompressed)
                    }
                    
                    CompressionAlgorithm::Lz4 => {
                        let decompressed = lz4_flex::decompress(&data, *expected_size as usize)
                            .context("LZ4 decompression failed")?;
                        Ok(decompressed)
                    }
                    
                    _ => bail!("Unsupported compression algorithm: {:?}", algorithm),
                }
            }
            
            PatchOperation::InsertHeader { data: header_data } => {
                let mut result = header_data.clone();
                result.extend_from_slice(&data);
                Ok(result)
            }
            
            PatchOperation::AppendFooter { data: footer_data } => {
                data.extend_from_slice(footer_data);
                Ok(data)
            }
            
            PatchOperation::Xor { key } => {
                for (i, byte) in data.iter_mut().enumerate() {
                    *byte ^= key[i % key.len()];
                }
                Ok(data)
            }
            
            PatchOperation::Convert { .. } => {
                // Format conversion would be implemented here
                // For now, return data unchanged
                Ok(data)
            }
        }
    }
}

/// Result of applying a patch recipe
#[derive(Debug, Clone)]
pub struct ApplyResult {
    /// Successfully applied files
    pub applied_files: Vec<AppliedFile>,
    /// Warnings encountered during application
    pub warnings: Vec<String>,
    /// Total application time
    pub duration_ms: u64,
    /// Recipe version used
    pub recipe_version: String,
}

/// Information about a successfully applied file
#[derive(Debug, Clone)]
pub struct AppliedFile {
    /// Output file path
    pub path: PathBuf,
    /// File size in bytes
    pub size_bytes: u64,
    /// Source delta name
    pub source_delta: String,
}

impl PatchRecipe {
    /// Save recipe to file
    pub fn save_to_file(&self, path: &Path) -> Result<()> {
        let json = serde_json::to_string_pretty(self)
            .context("Failed to serialize recipe")?;
        
        std::fs::write(path, json)
            .context("Failed to write recipe file")?;
        
        Ok(())
    }
    
    /// Load recipe from file
    pub fn load_from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .context("Failed to read recipe file")?;
        
        serde_json::from_str(&content)
            .context("Failed to parse recipe JSON")
    }
    
    /// Get summary information
    pub fn summary(&self) -> RecipeSummary {
        RecipeSummary {
            version: self.version.clone(),
            source_hash: self.source_hash.clone(),
            asset_count: self.deltas.len(),
            total_output_size: self.deltas.iter().map(|d| d.output_size).sum(),
            created_at: self.created_at,
        }
    }
}

/// Summary information about a patch recipe
#[derive(Debug, Clone)]
pub struct RecipeSummary {
    pub version: String,
    pub source_hash: String,
    pub asset_count: usize,
    pub total_output_size: u64,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs::File;
    use std::io::Write;
    
    #[test]
    fn test_recipe_builder() {
        let temp_dir = TempDir::new().unwrap();
        let source_path = temp_dir.path().join("source.dat");
        
        // Create a dummy source file
        let mut file = File::create(&source_path).unwrap();
        file.write_all(b"test data").unwrap();
        
        let builder = PatchRecipeBuilder::new(&source_path);
        
        // Test requires provenance to build
        let result = builder.build();
        assert!(result.is_err());
    }
    
    #[test]
    fn test_hash_calculation() {
        let temp_dir = TempDir::new().unwrap();
        let source_path = temp_dir.path().join("source.dat");
        
        let test_data = b"Hello, World!";
        std::fs::write(&source_path, test_data).unwrap();
        
        let builder = PatchRecipeBuilder::new(&source_path);
        let hash = builder.calculate_source_hash().unwrap();
        
        // Verify hash is consistent
        let expected_hash = blake3::hash(test_data).to_hex().to_string();
        assert_eq!(hash, expected_hash);
    }
}
