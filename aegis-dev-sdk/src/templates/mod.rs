//! Template engine for generating plugin projects

use anyhow::{Result, Context};
use handlebars::Handlebars;
use serde_json::json;
use std::collections::HashMap;
use std::path::Path;
use walkdir::WalkDir;
use tracing::{info, debug};

use crate::commands::new_project::ProjectConfig;

pub mod unity;
pub mod unreal;
pub mod minimal;
pub mod advanced;

pub use unity::*;
pub use unreal::*;
pub use minimal::*;
pub use advanced::*;

/// Template engine for generating plugin projects
pub struct TemplateEngine {
    handlebars: Handlebars<'static>,
}

impl TemplateEngine {
    /// Create a new template engine
    pub fn new() -> Self {
        let mut handlebars = Handlebars::new();
        
        // Register helpers
        handlebars.register_helper("uppercase", Box::new(uppercase_helper));
        handlebars.register_helper("snake_case", Box::new(snake_case_helper));
        handlebars.register_helper("camel_case", Box::new(camel_case_helper));
        handlebars.register_helper("has_feature", Box::new(has_feature_helper));
        
        Self { handlebars }
    }
    
    /// Generate a project from template
    pub async fn generate_project(&self, config: &ProjectConfig, output_dir: &Path) -> Result<()> {
        info!("Generating project from '{}' template", config.template);
        
        // Get template data
        let template_data = self.get_template_data(config)?;
        
        // Get template files based on template type
        let template_files = match config.template.as_str() {
            "unity" => get_unity_template_files(),
            "unreal" => get_unreal_template_files(),
            "minimal" => get_minimal_template_files(),
            "advanced" => get_advanced_template_files(),
            "custom" => get_custom_template_files(),
            _ => get_minimal_template_files(),
        };
        
        // Create output directory structure
        std::fs::create_dir_all(output_dir)
            .with_context(|| format!("Failed to create output directory: {}", output_dir.display()))?;
        
        // Generate files from templates
        for (template_path, content) in template_files {
            let output_path = output_dir.join(&template_path);
            
            // Create parent directories
            if let Some(parent) = output_path.parent() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
            }
            
            // Render template
            let rendered_content = self.handlebars.render_template(&content, &template_data)
                .with_context(|| format!("Failed to render template: {}", template_path))?;
            
            // Write file
            std::fs::write(&output_path, rendered_content)
                .with_context(|| format!("Failed to write file: {}", output_path.display()))?;
                
            debug!("Generated: {}", output_path.display());
        }
        
        // Copy static assets if any
        self.copy_template_assets(config, output_dir).await?;
        
        info!("Template generation complete");
        Ok(())
    }
    
    /// Get template data for rendering
    fn get_template_data(&self, config: &ProjectConfig) -> Result<serde_json::Value> {
        let data = json!({
            "name": config.name,
            "name_snake": to_snake_case(&config.name),
            "name_camel": to_camel_case(&config.name),
            "name_upper": config.name.to_uppercase().replace('-', "_"),
            "description": config.description,
            "author": config.author,
            "license": config.license,
            "engine": config.engine,
            "template": config.template,
            "supported_formats": config.supported_formats,
            "features": config.features,
            "development_tools": config.development_tools,
            "compliance_level": config.compliance_level,
            "test_framework": config.test_framework,
            "has_tests": config.development_tools.contains(&"unit-testing-framework".to_string()),
            "has_docs": config.development_tools.contains(&"documentation-generation".to_string()),
            "has_ci": config.development_tools.contains(&"ci-cd-configuration".to_string()),
            "has_benchmarks": config.development_tools.contains(&"performance-benchmarks".to_string()),
            "year": chrono::Utc::now().format("%Y").to_string(),
        });
        
        Ok(data)
    }
    
    /// Copy static template assets
    async fn copy_template_assets(&self, config: &ProjectConfig, output_dir: &Path) -> Result<()> {
        // This would copy non-templated files like binaries, images, etc.
        // For now, we'll just create some standard directories
        
        let dirs_to_create = vec![
            "src",
            "tests",
            "examples",
            "docs",
            "assets",
        ];
        
        for dir in dirs_to_create {
            let dir_path = output_dir.join(dir);
            if !dir_path.exists() {
                std::fs::create_dir_all(&dir_path)
                    .with_context(|| format!("Failed to create directory: {}", dir_path.display()))?;
            }
        }
        
        Ok(())
    }
}

/// Handlebars helper for uppercase
fn uppercase_helper(
    h: &handlebars::Helper,
    _: &Handlebars,
    _: &handlebars::Context,
    _: &mut handlebars::RenderContext,
    out: &mut dyn handlebars::Output,
) -> handlebars::HelperResult {
    let param = h.param(0).unwrap().value().as_str().unwrap_or("");
    out.write(&param.to_uppercase())?;
    Ok(())
}

/// Handlebars helper for snake_case
fn snake_case_helper(
    h: &handlebars::Helper,
    _: &Handlebars,
    _: &handlebars::Context,
    _: &mut handlebars::RenderContext,
    out: &mut dyn handlebars::Output,
) -> handlebars::HelperResult {
    let param = h.param(0).unwrap().value().as_str().unwrap_or("");
    out.write(&to_snake_case(param))?;
    Ok(())
}

/// Handlebars helper for camelCase
fn camel_case_helper(
    h: &handlebars::Helper,
    _: &Handlebars,
    _: &handlebars::Context,
    _: &mut handlebars::RenderContext,
    out: &mut dyn handlebars::Output,
) -> handlebars::HelperResult {
    let param = h.param(0).unwrap().value().as_str().unwrap_or("");
    out.write(&to_camel_case(param))?;
    Ok(())
}

/// Handlebars helper for checking features
fn has_feature_helper(
    h: &handlebars::Helper,
    _: &Handlebars,
    ctx: &handlebars::Context,
    _: &mut handlebars::RenderContext,
    out: &mut dyn handlebars::Output,
) -> handlebars::HelperResult {
    let feature = h.param(0).unwrap().value().as_str().unwrap_or("");
    let empty_features = vec![];
    let features = ctx.data()["features"].as_array().unwrap_or(&empty_features);
    
    let has_feature = features.iter().any(|f| f.as_str() == Some(feature));
    out.write(&has_feature.to_string())?;
    Ok(())
}

/// Convert string to snake_case
fn to_snake_case(s: &str) -> String {
    s.replace('-', "_").to_lowercase()
}

/// Convert string to camelCase
fn to_camel_case(s: &str) -> String {
    let words: Vec<&str> = s.split(&['-', '_']).collect();
    if words.is_empty() {
        return String::new();
    }
    
    let mut result = words[0].to_lowercase();
    for word in &words[1..] {
        if !word.is_empty() {
            let mut chars = word.chars();
            if let Some(first) = chars.next() {
                result.push(first.to_uppercase().next().unwrap());
                result.push_str(&chars.as_str().to_lowercase());
            }
        }
    }
    result
}