//! Minimal plugin template

use std::collections::HashMap;

/// Get minimal plugin template files
pub fn get_minimal_template_files() -> HashMap<String, String> {
    let mut files = HashMap::new();
    
    files.insert("Cargo.toml".to_string(), MINIMAL_CARGO_TOML.to_string());
    files.insert("src/lib.rs".to_string(), MINIMAL_LIB_RS.to_string());
    files.insert("README.md".to_string(), MINIMAL_README.to_string());
    files.insert(".gitignore".to_string(), GITIGNORE.to_string());
    
    files
}

const MINIMAL_CARGO_TOML: &str = r#"[package]
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
"#;

const MINIMAL_LIB_RS: &str = r#"//! {{description}}

use aegis_core::{
    plugin::{Plugin, PluginFactory, PluginInfo, PluginResult},
    resource::{Resource, ResourceType},
    compliance::ComplianceInfo,
    error::PluginError,
};
use async_trait::async_trait;
use std::path::Path;

/// Plugin factory
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

/// Main plugin implementation
pub struct {{name_camel}}Plugin;

impl {{name_camel}}Plugin {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Plugin for {{name_camel}}Plugin {
    async fn can_handle(&self, file_path: &Path, file_header: &[u8]) -> PluginResult<bool> {
        // TODO: Implement file format detection
        Ok(false)
    }
    
    async fn extract_resources(&mut self, file_path: &Path) -> PluginResult<Vec<Resource>> {
        // TODO: Implement resource extraction
        Ok(Vec::new())
    }
    
    async fn list_entries(&self, file_path: &Path) -> PluginResult<Vec<aegis_core::archive::ArchiveEntry>> {
        // TODO: Implement entry listing
        Ok(Vec::new())
    }
}

// Export the plugin factory
aegis_core::export_plugin!({{name_camel}}Factory);
"#;

const MINIMAL_README: &str = r#"# {{name}}

{{description}}

## Quick Start

```rust
use {{name_snake}}::*;
use aegis_core::plugin::{Plugin, PluginFactory};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let factory = {{name_camel}}Factory;
    let mut plugin = factory.create_plugin();
    
    // TODO: Use your plugin here
    
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