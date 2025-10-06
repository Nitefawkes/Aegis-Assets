//! Interactive project scaffolding and template system

use anyhow::{Result, Context};
use colored::*;
use dialoguer::{Input, Select, Confirm, MultiSelect};
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use tracing::{info, warn};

use crate::templates::TemplateEngine;
use crate::scaffold::ProjectScaffold;

/// Create a new plugin project with interactive setup
pub async fn create_new_project(
    name: String,
    template: String,
    engine: Option<String>,
    no_interactive: bool,
    output: Option<PathBuf>,
) -> Result<()> {
    println!("{}", "🚀 Creating new Aegis plugin project...".bold().green());
    
    // Validate project name
    if !is_valid_project_name(&name) {
        return Err(anyhow::anyhow!(
            "Invalid project name: '{}'. Must be a valid Rust crate name (lowercase, alphanumeric, hyphens)",
            name
        ));
    }
    
    // Determine output directory
    let output_dir = output.unwrap_or_else(|| PathBuf::from(&name));
    
    if output_dir.exists() && !no_interactive {
        let overwrite = Confirm::new()
            .with_prompt(&format!("Directory '{}' already exists. Overwrite?", output_dir.display()))
            .default(false)
            .interact()?;
            
        if !overwrite {
            println!("❌ Cancelled project creation");
            return Ok(());
        }
    }
    
    // Get project configuration
    let config = if no_interactive {
        get_default_config(name, template, engine)
    } else {
        get_interactive_config(name, template, engine).await?
    };
    
    // Create project scaffold
    let scaffold = ProjectScaffold::new(config.clone());
    scaffold.create_project(&output_dir).await?;
    
    // Generate from template
    let template_engine = TemplateEngine::new();
    template_engine.generate_project(&config, &output_dir).await?;
    
    // Post-creation steps
    run_post_creation_steps(&config, &output_dir).await?;
    
    println!("\n{}", "✅ Project created successfully!".bold().green());
    print_next_steps(&config, &output_dir);
    
    Ok(())
}

/// Project configuration structure
#[derive(Debug, Clone)]
pub struct ProjectConfig {
    pub name: String,
    pub template: String,
    pub engine: String,
    pub description: String,
    pub author: String,
    pub license: String,
    pub supported_formats: Vec<String>,
    pub features: Vec<String>,
    pub development_tools: Vec<String>,
    pub compliance_level: String,
    pub test_framework: String,
}

/// Get configuration interactively
async fn get_interactive_config(
    name: String, 
    template: String, 
    engine: Option<String>
) -> Result<ProjectConfig> {
    println!("{}", "📋 Project Configuration".bold().blue());
    println!("Let's set up your new plugin project with a few questions...\n");
    
    // Template selection
    let template = if template == "interactive" {
        let templates = vec![
            "Unity Asset Bundle Plugin",
            "Unreal Engine Archive Plugin", 
            "Custom Game Engine Plugin",
            "Minimal Plugin Template",
            "Advanced Plugin with Tests",
        ];
        
        let selection = Select::new()
            .with_prompt("Choose a template")
            .items(&templates)
            .default(0)
            .interact()?;
            
        match selection {
            0 => "unity".to_string(),
            1 => "unreal".to_string(),
            2 => "custom".to_string(),
            3 => "minimal".to_string(),
            4 => "advanced".to_string(),
            _ => "minimal".to_string(),
        }
    } else {
        template
    };
    
    // Engine selection (if not specified)
    let engine = engine.unwrap_or_else(|| {
        if let Ok(engine_idx) = Select::new()
            .with_prompt("Target game engine")
            .items(&["Unity", "Unreal Engine", "Custom/Other"])
            .default(0)
            .interact() {
            match engine_idx {
                0 => "unity".to_string(),
                1 => "unreal".to_string(),
                _ => "other".to_string(),
            }
        } else {
            "unity".to_string()
        }
    });
    
    // Basic project info
    let description: String = Input::new()
        .with_prompt("Project description")
        .default("A compliance-first asset extraction plugin for Aegis-Assets".to_string())
        .interact_text()?;
        
    let author: String = Input::new()
        .with_prompt("Author name")
        .default(get_git_user_name().unwrap_or_else(|| "Your Name".to_string()))
        .interact_text()?;
        
    let license_options = vec!["MIT", "Apache-2.0", "GPL-3.0", "BSD-3-Clause"];
    let license_idx = Select::new()
        .with_prompt("License")
        .items(&license_options)
        .default(0)
        .interact()?;
    let license = license_options[license_idx].to_string();
    
    // Supported formats
    let format_options = match engine.as_str() {
        "unity" => vec![
            "Unity Asset Bundles (.ab)",
            "Unity Scenes (.unity)",
            "Unity Packages (.unitypackage)",
            "Unity Resources (.assets)",
        ],
        "unreal" => vec![
            "Unreal Packages (.upk)",
            "Unreal Archives (.pak)",
            "Unreal Maps (.umap)",
            "Unreal Assets (.uasset)",
        ],
        _ => vec![
            "Custom Archive Format",
            "Binary Resource Files",
            "Compressed Packages",
            "Proprietary Format",
        ],
    };
    
    let format_selections = MultiSelect::new()
        .with_prompt("Supported file formats (select multiple)")
        .items(&format_options)
        .interact()?;
        
    let supported_formats = format_selections
        .into_iter()
        .map(|i| extract_extension(&format_options[i]))
        .collect();
    
    // Features
    let feature_options = vec![
        "Asset Preview Generation",
        "Batch Processing",
        "Format Conversion",
        "Metadata Extraction",
        "Compression Support",
        "Encryption Handling",
        "Streaming Support",
        "Memory Optimization",
    ];
    
    let feature_selections = MultiSelect::new()
        .with_prompt("Plugin features (optional)")
        .items(&feature_options)
        .interact()?;
        
    let features = feature_selections
        .into_iter()
        .map(|i| slugify(&feature_options[i]))
        .collect();
    
    // Development tools
    let dev_tool_options = vec![
        "Unit Testing Framework",
        "Integration Tests",
        "Performance Benchmarks",
        "Documentation Generation",
        "Code Coverage",
        "Linting and Formatting",
        "CI/CD Configuration",
    ];
    
    let dev_tool_selections = MultiSelect::new()
        .with_prompt("Development tools (recommended)")
        .items(&dev_tool_options)
        .interact()?;
        
    let development_tools: Vec<String> = dev_tool_selections
        .into_iter()
        .map(|i| slugify(&dev_tool_options[i]))
        .collect();
    
    // Compliance level
    let compliance_options = vec!["Basic", "Standard", "Enterprise"];
    let compliance_idx = Select::new()
        .with_prompt("Compliance level")
        .items(&compliance_options)
        .default(1)
        .interact()?;
    let compliance_level = compliance_options[compliance_idx].to_lowercase();
    
    // Test framework
    let test_framework = if development_tools.contains(&"unit-testing-framework".to_string()) {
        let test_options = vec!["Built-in", "Custom Test Suite", "Property-based Testing"];
        let test_idx = Select::new()
            .with_prompt("Test framework")
            .items(&test_options)
            .default(0)
            .interact()?;
        slugify(&test_options[test_idx])
    } else {
        "none".to_string()
    };
    
    Ok(ProjectConfig {
        name,
        template,
        engine,
        description,
        author,
        license,
        supported_formats,
        features,
        development_tools,
        compliance_level,
        test_framework,
    })
}

/// Get default configuration (non-interactive)
fn get_default_config(name: String, template: String, engine: Option<String>) -> ProjectConfig {
    ProjectConfig {
        name,
        template: if template == "interactive" { "minimal".to_string() } else { template },
        engine: engine.unwrap_or_else(|| "unity".to_string()),
        description: "A compliance-first asset extraction plugin for Aegis-Assets".to_string(),
        author: get_git_user_name().unwrap_or_else(|| "Your Name".to_string()),
        license: "MIT".to_string(),
        supported_formats: vec![".ab".to_string()],
        features: vec![],
        development_tools: vec!["unit-testing-framework".to_string()],
        compliance_level: "standard".to_string(),
        test_framework: "built-in".to_string(),
    }
}

/// Run post-creation setup steps
async fn run_post_creation_steps(config: &ProjectConfig, output_dir: &Path) -> Result<()> {
    info!("Running post-creation setup...");
    
    // Initialize git repository
    if !output_dir.join(".git").exists() {
        if let Err(e) = std::process::Command::new("git")
            .arg("init")
            .current_dir(output_dir)
            .output() {
            warn!("Failed to initialize git repository: {}", e);
        } else {
            info!("Initialized git repository");
        }
    }
    
    // Run cargo check to ensure project compiles
    println!("🔧 Checking project compilation...");
    let output = std::process::Command::new("cargo")
        .args(&["check", "--quiet"])
        .current_dir(output_dir)
        .output()
        .context("Failed to run cargo check")?;
        
    if !output.status.success() {
        warn!("Project compilation check failed:");
        warn!("{}", String::from_utf8_lossy(&output.stderr));
    } else {
        info!("Project compiles successfully");
    }
    
    // Generate initial documentation if enabled
    if config.development_tools.contains(&"documentation-generation".to_string()) {
        info!("Generating initial documentation...");
        // This would call the docs generation
    }
    
    Ok(())
}

/// Print next steps for the user
fn print_next_steps(config: &ProjectConfig, output_dir: &Path) {
    println!("\n{}", "🎯 Next Steps:".bold().yellow());
    println!("  1. 📁 cd {}", output_dir.display());
    println!("  2. 🔧 cargo build");
    println!("  3. 🧪 cargo test");
    
    if config.development_tools.contains(&"unit-testing-framework".to_string()) {
        println!("  4. 🚀 aegis-dev dev    # Start development server");
    }
    
    println!("\n{}", "📚 Documentation:".bold().blue());
    println!("  • Plugin API: https://docs.aegis-assets.org/plugin-api");
    println!("  • Examples: ./examples/");
    println!("  • Tests: ./tests/");
    
    if config.development_tools.contains(&"documentation-generation".to_string()) {
        println!("  • Local docs: aegis-dev docs serve");
    }
    
    println!("\n{}", "🛡️ Security & Compliance:".bold().cyan());
    println!("  • Validate: aegis-dev validate .");
    println!("  • Sign: aegis-dev sign plugin target/release/lib{}.rlib", config.name.replace("-", "_"));
    println!("  • Publish: aegis-dev market publish");
}

/// Validate project name
fn is_valid_project_name(name: &str) -> bool {
    !name.is_empty() 
        && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
        && !name.starts_with('-')
        && !name.ends_with('-')
}

/// Get git user name
fn get_git_user_name() -> Option<String> {
    std::process::Command::new("git")
        .args(&["config", "--get", "user.name"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout).ok()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
            } else {
                None
            }
        })
}

/// Extract file extension from format description
fn extract_extension(format_desc: &str) -> String {
    if let Some(start) = format_desc.find('(') {
        if let Some(end) = format_desc.find(')') {
            return format_desc[start+1..end].to_string();
        }
    }
    format_desc.to_lowercase().replace(' ', "-")
}

/// Convert to slug format
fn slugify(text: &str) -> String {
    text.to_lowercase()
        .replace(' ', "-")
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect()
}