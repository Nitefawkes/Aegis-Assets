//! Unreal Engine plugin template

use std::collections::HashMap;

/// Get Unreal plugin template files  
pub fn get_unreal_template_files() -> HashMap<String, String> {
    let mut files = HashMap::new();
    
    files.insert("Cargo.toml".to_string(), UNREAL_CARGO_TOML.to_string());
    files.insert("src/lib.rs".to_string(), UNREAL_LIB_RS.to_string());
    files.insert("README.md".to_string(), UNREAL_README.to_string());
    files.insert(".gitignore".to_string(), GITIGNORE.to_string());
    
    files
}

const UNREAL_CARGO_TOML: &str = r#"[package]
name = "{{name}}"
version = "0.1.0" 
edition = "2021"
authors = ["{{author}}"]
license = "{{license}}"
description = "{{description}}"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
aegis-core = { version = "0.1", features = ["plugin-api"] }
serde = { version = "1.0", features = ["derive"] }
anyhow = "1.0"
async-trait = "0.1"
tracing = "0.1"
byteorder = "1.5"
"#;

const UNREAL_LIB_RS: &str = r#"//! {{description}}

use aegis_core::{
    plugin::{Plugin, PluginFactory, PluginInfo, PluginResult},
    resource::{Resource, ResourceType},
    compliance::ComplianceInfo,
    error::PluginError,
};
use async_trait::async_trait;
use std::path::Path;

/// Unreal plugin factory
pub struct {{name_camel}}Factory;

impl PluginFactory for {{name_camel}}Factory {
    fn create_plugin(&self) -> Box<dyn Plugin> {
        Box::new({{name_camel}}Plugin::new())
    }
    
    fn plugin_info(&self) -> PluginInfo {
        PluginInfo {
            name: "{{name}}".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            description: "{{description}}".to_string(),
            author: Some("{{author}}".to_string()),
            supported_extensions: vec![
                {{#each supported_formats}}
                "{{this}}".to_string(),
                {{/each}}
            ],
            compliance_info: ComplianceInfo::default(),
        }
    }
}

/// Main Unreal plugin implementation
pub struct {{name_camel}}Plugin;

impl {{name_camel}}Plugin {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Plugin for {{name_camel}}Plugin {
    async fn can_handle(&self, file_path: &Path, file_header: &[u8]) -> PluginResult<bool> {
        // Check for Unreal package signature
        if file_header.len() >= 4 {
            // Check for UE4/UE5 package signature
            if &file_header[0..4] == &[0x9E, 0x2A, 0x83, 0xC1] {
                return Ok(true);
            }
        }
        
        // Check file extension
        if let Some(ext) = file_path.extension().and_then(|e| e.to_str()) {
            match ext.to_lowercase().as_str() {
                {{#each supported_formats}}
                "{{this}}" => Ok(true),
                {{/each}}
                _ => Ok(false),
            }
        } else {
            Ok(false)
        }
    }
    
    async fn extract_resources(&mut self, file_path: &Path) -> PluginResult<Vec<Resource>> {
        // TODO: Implement Unreal asset extraction
        Ok(Vec::new())
    }
    
    async fn list_entries(&self, file_path: &Path) -> PluginResult<Vec<aegis_core::archive::ArchiveEntry>> {
        // TODO: Implement Unreal package entry listing
        Ok(Vec::new())
    }
}

// Export the plugin factory
aegis_core::export_plugin!({{name_camel}}Factory);
"#;

const UNREAL_README: &str = r#"# {{name}}

{{description}}

## Supported Unreal Formats

{{#each supported_formats}}
- `{{this}}` - Unreal Engine format
{{/each}}

## Quick Start

```rust
use {{name_snake}}::*;
use aegis_core::plugin::{Plugin, PluginFactory};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let factory = {{name_camel}}Factory;
    let mut plugin = factory.create_plugin();
    
    let resources = plugin.extract_resources("MyGame.pak").await?;
    
    for resource in resources {
        println!("Extracted: {} ({} bytes)", resource.name, resource.size);
    }
    
    Ok(())
}
```

## License

Licensed under {{license}}.
"#;

const GITIGNORE: &str = r#"/target/
**/*.rs.bk
Cargo.lock
.DS_Store
"#;