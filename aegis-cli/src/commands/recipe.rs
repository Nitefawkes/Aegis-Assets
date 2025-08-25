use anyhow::{Result, Context};
use clap::{Args, Subcommand};
use std::path::PathBuf;
use aegis_core::{AegisCore, patch::{PatchRecipe, PatchApplier, PatchRecipeBuilder}};
use crate::ui::{progress_bar, success, warning, error, info};
use colored::*;

/// Work with patch recipes
#[derive(Args)]
pub struct RecipeCommand {
    #[command(subcommand)]
    pub action: RecipeAction,
}

#[derive(Subcommand)]
pub enum RecipeAction {
    /// Create a patch recipe from extracted assets
    Create {
        /// Source game file
        #[arg(short, long)]
        source: PathBuf,
        
        /// Directory containing extracted assets
        #[arg(short, long)]
        assets: PathBuf,
        
        /// Output recipe file
        #[arg(short, long)]
        output: PathBuf,
        
        /// Include detailed metadata
        #[arg(long)]
        detailed: bool,
    },
    
    /// Apply a patch recipe to recreate assets
    Apply {
        /// Patch recipe file
        #[arg(short, long)]
        recipe: PathBuf,
        
        /// Source game file (must match recipe)
        #[arg(short, long)]
        source: PathBuf,
        
        /// Output directory for recreated assets
        #[arg(short, long)]
        output: PathBuf,
        
        /// Verify checksums after recreation
        #[arg(long)]
        verify: bool,
    },
    
    /// Show information about a patch recipe
    Info {
        /// Patch recipe file
        recipe: PathBuf,
        
        /// Show detailed asset information
        #[arg(long)]
        detailed: bool,
    },
    
    /// Validate a patch recipe
    Validate {
        /// Patch recipe file
        recipe: PathBuf,
        
        /// Source file to validate against (optional)
        #[arg(long)]
        source: Option<PathBuf>,
    },
}

impl RecipeCommand {
    pub fn execute(&self, core: &AegisCore) -> Result<()> {
        match &self.action {
            RecipeAction::Create { source, assets, output, detailed } => {
                self.create_recipe(source, assets, output, *detailed)
            }
            RecipeAction::Apply { recipe, source, output, verify } => {
                self.apply_recipe(recipe, source, output, *verify)
            }
            RecipeAction::Info { recipe, detailed } => {
                self.show_recipe_info(recipe, *detailed)
            }
            RecipeAction::Validate { recipe, source } => {
                self.validate_recipe(recipe, source.as_ref())
            }
        }
    }
    
    fn create_recipe(&self, source: &PathBuf, assets: &PathBuf, output: &PathBuf, detailed: bool) -> Result<()> {
        info(&format!("Creating patch recipe from: {}", source.display()));
        
        // Validate inputs
        if !source.exists() {
            return Err(anyhow::anyhow!("Source file does not exist: {}", source.display()));
        }
        
        if !assets.exists() {
            return Err(anyhow::anyhow!("Assets directory does not exist: {}", assets.display()));
        }
        
        // This is a placeholder implementation
        // Real implementation would:
        // 1. Analyze the source file
        // 2. Compare with extracted assets
        // 3. Generate delta patches
        // 4. Create recipe with provenance
        
        warning("Recipe creation is not fully implemented yet");
        
        // Mock recipe creation
        println!("{}", "Recipe Creation Process:".bright_blue().bold());
        println!("  1. Analyzing source file...");
        println!("  2. Scanning asset directory...");
        println!("  3. Computing delta patches...");
        println!("  4. Generating recipe...");
        
        // Create a basic recipe structure
        let recipe_content = serde_json::json!({
            "version": "1.0",
            "source_file": source.file_name().unwrap().to_string_lossy(),
            "created_at": chrono::Utc::now().to_rfc3339(),
            "note": "This is a placeholder recipe - full implementation pending"
        });
        
        std::fs::write(output, serde_json::to_string_pretty(&recipe_content)?)
            .context("Failed to write recipe file")?;
        
        success(&format!("Recipe created: {}", output.display()));
        Ok(())
    }
    
    fn apply_recipe(&self, recipe_path: &PathBuf, source: &PathBuf, output: &PathBuf, verify: bool) -> Result<()> {
        info(&format!("Applying patch recipe: {}", recipe_path.display()));
        
        // Load recipe
        let applier = PatchApplier::from_file(recipe_path)
            .context("Failed to load patch recipe")?;
        
        // Apply recipe
        println!("{}", "Applying Recipe:".bright_blue().bold());
        println!("  Recipe: {}", recipe_path.display());
        println!("  Source: {}", source.display());
        println!("  Output: {}", output.display());
        
        let result = applier.apply(source, output)
            .context("Failed to apply recipe")?;
        
        // Display results
        println!("\n{}", "Application Results:".bright_green().bold());
        println!("  Files created: {}", result.applied_files.len());
        println!("  Duration: {} ms", result.duration_ms);
        println!("  Recipe version: {}", result.recipe_version);
        
        if !result.warnings.is_empty() {
            println!("\n{}", "Warnings:".yellow().bold());
            for warning in &result.warnings {
                println!("  ⚠ {}", warning.yellow());
            }
        }
        
        if verify {
            info("Verifying recreated assets...");
            self.verify_recreated_assets(&result.applied_files)?;
        }
        
        success("Recipe applied successfully!");
        Ok(())
    }
    
    fn show_recipe_info(&self, recipe_path: &PathBuf, detailed: bool) -> Result<()> {
        info(&format!("Loading recipe: {}", recipe_path.display()));
        
        let recipe = PatchRecipe::load_from_file(recipe_path)
            .context("Failed to load recipe")?;
        
        let summary = recipe.summary();
        
        println!("{}", "Recipe Information:".bright_blue().bold());
        println!("  Version: {}", summary.version);
        println!("  Source hash: {}", summary.source_hash);
        println!("  Assets: {}", summary.asset_count);
        println!("  Total output size: {} bytes", summary.total_output_size);
        println!("  Created: {}", summary.created_at.format("%Y-%m-%d %H:%M:%S UTC"));
        
        if detailed {
            // Show additional details about the recipe
            println!("\n{}", "Detailed Information:".bright_yellow().bold());
            println!("  Provenance:");
            println!("    Game ID: {}", recipe.provenance.game_id.as_deref().unwrap_or("Unknown"));
            println!("    Extraction time: {}", recipe.provenance.extraction_time.format("%Y-%m-%d %H:%M:%S UTC"));
            println!("    Aegis version: {}", recipe.provenance.aegis_version);
            
            println!("\n  Compliance:");
            println!("    Publisher: {}", recipe.provenance.compliance_profile.publisher);
            println!("    Enforcement level: {:?}", recipe.provenance.compliance_profile.enforcement_level);
            
            if !recipe.deltas.is_empty() {
                println!("\n  Delta Patches:");
                for (i, delta) in recipe.deltas.iter().take(5).enumerate() {
                    println!("    {}: {} ({} → {} bytes)", 
                        i + 1, 
                        delta.target_file,
                        delta.source_length,
                        delta.output_size
                    );
                }
                
                if recipe.deltas.len() > 5 {
                    println!("    ... and {} more", recipe.deltas.len() - 5);
                }
            }
        }
        
        Ok(())
    }
    
    fn validate_recipe(&self, recipe_path: &PathBuf, source: Option<&PathBuf>) -> Result<()> {
        info(&format!("Validating recipe: {}", recipe_path.display()));
        
        // Load recipe
        let recipe = PatchRecipe::load_from_file(recipe_path)
            .context("Failed to load recipe")?;
        
        println!("{}", "Recipe Validation:".bright_blue().bold());
        
        // Basic validation
        let mut issues = Vec::new();
        
        // Check version compatibility
        if recipe.version != "1.0" {
            issues.push(format!("Unsupported recipe version: {}", recipe.version));
        }
        
        // Check if deltas exist
        if recipe.deltas.is_empty() {
            issues.push("Recipe contains no delta patches".to_string());
        }
        
        // Validate each delta
        for (i, delta) in recipe.deltas.iter().enumerate() {
            if delta.target_file.is_empty() {
                issues.push(format!("Delta {} has empty target file name", i));
            }
            
            if delta.output_size == 0 {
                issues.push(format!("Delta {} has zero output size", i));
            }
        }
        
        // If source file provided, validate against it
        if let Some(source_path) = source {
            info("Validating against source file...");
            
            if !source_path.exists() {
                issues.push(format!("Source file does not exist: {}", source_path.display()));
            } else {
                // Check file size
                let metadata = std::fs::metadata(source_path)?;
                if metadata.len() != recipe.source_size {
                    issues.push(format!(
                        "Source file size mismatch: expected {}, got {}",
                        recipe.source_size,
                        metadata.len()
                    ));
                }
                
                // Check hash (this would be expensive for large files)
                info("Verifying source file hash (this may take a moment)...");
                let file_data = std::fs::read(source_path)?;
                let calculated_hash = blake3::hash(&file_data).to_hex().to_string();
                
                if calculated_hash != recipe.source_hash {
                    issues.push(format!(
                        "Source file hash mismatch: expected {}, got {}",
                        recipe.source_hash,
                        calculated_hash
                    ));
                }
            }
        }
        
        // Display results
        if issues.is_empty() {
            success("Recipe validation passed!");
        } else {
            error(&format!("Recipe validation failed with {} issues:", issues.len()));
            for (i, issue) in issues.iter().enumerate() {
                println!("  {}: {}", i + 1, issue.red());
            }
        }
        
        Ok(())
    }
    
    fn verify_recreated_assets(&self, applied_files: &[aegis_core::patch::AppliedFile]) -> Result<()> {
        println!("{}", "Asset Verification:".bright_blue().bold());
        
        let mut verified = 0;
        let mut failed = 0;
        
        for file in applied_files {
            if file.path.exists() {
                let metadata = std::fs::metadata(&file.path)?;
                if metadata.len() == file.size_bytes {
                    verified += 1;
                } else {
                    failed += 1;
                    warning(&format!(
                        "Size mismatch for {}: expected {}, got {}",
                        file.path.display(),
                        file.size_bytes,
                        metadata.len()
                    ));
                }
            } else {
                failed += 1;
                error(&format!("Missing file: {}", file.path.display()));
            }
        }
        
        println!("  {} verified", verified.to_string().bright_green());
        if failed > 0 {
            println!("  {} failed", failed.to_string().red());
        }
        
        Ok(())
    }
}
