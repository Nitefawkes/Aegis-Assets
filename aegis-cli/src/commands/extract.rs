use anyhow::{Result, Context};
use clap::{Args, ValueEnum};
use std::path::PathBuf;
use aegis_core::{AegisCore, ComplianceLevel};
use crate::ui::{progress_bar, success, warning, error, info};
use indicatif::{ProgressBar, ProgressStyle};

/// Extract assets from game files
#[derive(Args)]
pub struct ExtractCommand {
    /// Input file or directory to extract from
    #[arg(short, long)]
    pub input: PathBuf,
    
    /// Output directory for extracted assets
    #[arg(short, long)]
    pub output: PathBuf,
    
    /// Engine/format hint (auto-detect if not specified)
    #[arg(short, long)]
    pub engine: Option<SupportedEngine>,
    
    /// Asset types to extract (all if not specified)
    #[arg(long, value_delimiter = ',')]
    pub types: Option<Vec<AssetType>>,
    
    /// Enable compression for exported textures
    #[arg(long)]
    pub compress_textures: bool,
    
    /// Texture compression quality (0-255)
    #[arg(long, default_value = "128")]
    pub texture_quality: u8,
    
    /// Generate patch recipe instead of direct extraction
    #[arg(long)]
    pub recipe_only: bool,
    
    /// Skip compliance warnings (not recommended)
    #[arg(long)]
    pub skip_warnings: bool,
    
    /// Overwrite existing output files
    #[arg(long)]
    pub overwrite: bool,
}

#[derive(ValueEnum, Clone, Debug)]
pub enum SupportedEngine {
    Unity,
    Unreal,
    Source,
    Bethesda,
    Auto,
}

#[derive(ValueEnum, Clone, Debug)]
pub enum AssetType {
    Textures,
    Meshes,
    Materials,
    Audio,
    Animations,
    Levels,
    All,
}

impl ExtractCommand {
    pub fn execute(&self, core: &AegisCore) -> Result<()> {
        info(&format!("Starting extraction from: {}", self.input.display()));
        
        // Validate inputs
        self.validate()?;
        
        // Create output directory
        if !self.output.exists() {
            std::fs::create_dir_all(&self.output)
                .context("Failed to create output directory")?;
        }
        
        // Create extractor
        let mut extractor = core.create_extractor();
        
        // Perform extraction
        if self.input.is_file() {
            self.extract_single_file(&mut extractor)?;
        } else {
            self.extract_directory(&mut extractor)?;
        }
        
        success("Extraction completed successfully!");
        Ok(())
    }
    
    fn validate(&self) -> Result<()> {
        // Check if input exists
        if !self.input.exists() {
            return Err(anyhow::anyhow!("Input path does not exist: {}", self.input.display()));
        }
        
        // Check if output directory is writable
        if self.output.exists() && !self.overwrite {
            let entries = std::fs::read_dir(&self.output)?;
            if entries.count() > 0 {
                return Err(anyhow::anyhow!(
                    "Output directory is not empty. Use --overwrite to overwrite existing files."
                ));
            }
        }
        
        // Validate texture quality range
        if self.texture_quality > 255 {
            return Err(anyhow::anyhow!("Texture quality must be between 0 and 255"));
        }
        
        Ok(())
    }
    
    fn extract_single_file(&self, extractor: &mut aegis_core::extract::Extractor) -> Result<()> {
        info(&format!("Extracting file: {}", self.input.display()));
        
        let pb = progress_bar(0);
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.green} {msg}")
                .unwrap()
        );
        pb.set_message("Analyzing file format...");
        
        // Extract the file
        let result = extractor.extract_from_file(&self.input, &self.output)
            .context("Extraction failed")?;
        
        pb.finish_and_clear();
        
        // Handle compliance warnings
        self.handle_compliance_result(&result)?;
        
        // Display results
        self.display_results(&result);
        
        Ok(())
    }
    
    fn extract_directory(&self, extractor: &mut aegis_core::extract::Extractor) -> Result<()> {
        info(&format!("Extracting directory: {}", self.input.display()));
        
        // Find all supported files in directory
        let files = self.find_supported_files(&self.input)?;
        
        if files.is_empty() {
            warning("No supported files found in directory");
            return Ok(());
        }
        
        info(&format!("Found {} supported files", files.len()));
        
        let pb = ProgressBar::new(files.len() as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
                .unwrap()
        );
        
        let mut successful = 0;
        let mut failed = 0;
        
        for file in files {
            pb.set_message(format!("Processing {}", file.file_name().unwrap_or_default().to_string_lossy()));
            
            match extractor.extract_from_file(&file, &self.output) {
                Ok(result) => {
                    successful += 1;
                    if let Err(e) = self.handle_compliance_result(&result) {
                        warning(&format!("Compliance warning for {}: {}", file.display(), e));
                    }
                }
                Err(e) => {
                    failed += 1;
                    error(&format!("Failed to extract {}: {}", file.display(), e));
                }
            }
            
            pb.inc(1);
        }
        
        pb.finish_and_clear();
        
        // Summary
        println!("\n{}", "Extraction Summary:".bright_yellow().bold());
        println!("  {} successful", successful.to_string().bright_green());
        if failed > 0 {
            println!("  {} failed", failed.to_string().red());
        }
        
        Ok(())
    }
    
    fn find_supported_files(&self, dir: &std::path::Path) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_file() {
                // Check file extension
                if let Some(ext) = path.extension() {
                    let ext_str = ext.to_string_lossy().to_lowercase();
                    if self.is_supported_extension(&ext_str) {
                        files.push(path);
                    }
                }
            } else if path.is_dir() {
                // Recursively search subdirectories
                files.extend(self.find_supported_files(&path)?);
            }
        }
        
        Ok(files)
    }
    
    fn is_supported_extension(&self, ext: &str) -> bool {
        matches!(ext, 
            "unity3d" | "assets" | "sharedassets" | "resource" | "ress" |  // Unity
            "pak" | "uasset" | "umap" | "ubulk" | "utoc" | "ucas" |        // Unreal
            "vpk" | "bsp" | "vtf" | "vmt" | "mdl" |                       // Source
            "ba2" | "bsa" | "esp" | "esm" | "nif"                         // Bethesda
        )
    }
    
    fn handle_compliance_result(&self, result: &aegis_core::extract::ExtractionResult) -> Result<()> {
        let compliance = &result.compliance_info;
        
        match compliance.risk_level {
            ComplianceLevel::Permissive => {
                // All good, no warnings needed
            }
            
            ComplianceLevel::Neutral => {
                if !self.skip_warnings {
                    warning("Compliance: Publisher policy unclear");
                    for warning in &compliance.warnings {
                        println!("  ⚠  {}", warning.yellow());
                    }
                    
                    if !compliance.recommendations.is_empty() {
                        println!("  💡 Recommendations:");
                        for rec in &compliance.recommendations {
                            println!("    • {}", rec.dimmed());
                        }
                    }
                }
            }
            
            ComplianceLevel::HighRisk => {
                if self.skip_warnings {
                    warning("⚠  HIGH RISK: Proceeding despite compliance warnings");
                } else {
                    error("🚫 HIGH RISK: This extraction may violate publisher policies");
                    
                    for warning in &compliance.warnings {
                        println!("  ⚠  {}", warning.red().bold());
                    }
                    
                    // Ask for explicit consent
                    println!("\n{}", "This operation requires explicit consent.".yellow().bold());
                    println!("Do you understand the risks and wish to proceed? (y/N)");
                    
                    let mut input = String::new();
                    std::io::stdin().read_line(&mut input)?;
                    
                    if !matches!(input.trim().to_lowercase().as_str(), "y" | "yes") {
                        return Err(anyhow::anyhow!("Extraction cancelled by user"));
                    }
                    
                    warning("Proceeding with high-risk extraction...");
                }
            }
        }
        
        Ok(())
    }
    
    fn display_results(&self, result: &aegis_core::extract::ExtractionResult) {
        println!("\n{}", "Extraction Results:".bright_blue().bold());
        println!("  Source: {}", result.source_path.display());
        println!("  Resources extracted: {}", result.resources.len().to_string().bright_green());
        
        if !result.warnings.is_empty() {
            println!("  Warnings: {}", result.warnings.len().to_string().yellow());
            for warning in &result.warnings {
                println!("    ⚠ {}", warning.yellow());
            }
        }
        
        println!("\n{}", "Performance:".bright_blue().bold());
        println!("  Duration: {} ms", result.metrics.duration_ms);
        println!("  Peak memory: {} MB", result.metrics.peak_memory_mb);
        println!("  Bytes extracted: {} bytes", result.metrics.bytes_extracted);
        
        // Show resource breakdown
        let mut resource_counts = std::collections::HashMap::new();
        for resource in &result.resources {
            *resource_counts.entry(resource.resource_type()).or_insert(0) += 1;
        }
        
        if !resource_counts.is_empty() {
            println!("\n{}", "Resource Breakdown:".bright_blue().bold());
            for (resource_type, count) in resource_counts {
                println!("  {}: {}", resource_type, count.to_string().bright_green());
            }
        }
    }
}

use colored::*;
