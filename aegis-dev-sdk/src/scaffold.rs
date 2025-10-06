//! Project scaffolding system

use anyhow::{Result, Context};
use std::path::Path;
use tracing::{info, debug};

use crate::commands::new_project::ProjectConfig;

/// Project scaffold builder
pub struct ProjectScaffold {
    config: ProjectConfig,
}

impl ProjectScaffold {
    pub fn new(config: ProjectConfig) -> Self {
        Self { config }
    }
    
    /// Create the project structure
    pub async fn create_project(&self, output_dir: &Path) -> Result<()> {
        info!("Creating project scaffold: {}", output_dir.display());
        
        // Create directory structure
        self.create_directory_structure(output_dir).await?;
        
        // Initialize git repository
        self.init_git_repository(output_dir).await?;
        
        // Set up development environment
        self.setup_dev_environment(output_dir).await?;
        
        info!("Project scaffold created successfully");
        Ok(())
    }
    
    async fn create_directory_structure(&self, output_dir: &Path) -> Result<()> {
        let directories = [
            "src",
            "tests", 
            "examples",
            "docs",
            "assets",
            "test-data",
            ".github/workflows",
            "scripts",
        ];
        
        for dir in &directories {
            let dir_path = output_dir.join(dir);
            std::fs::create_dir_all(&dir_path)
                .with_context(|| format!("Failed to create directory: {}", dir_path.display()))?;
            debug!("Created directory: {}", dir_path.display());
        }
        
        Ok(())
    }
    
    async fn init_git_repository(&self, output_dir: &Path) -> Result<()> {
        if output_dir.join(".git").exists() {
            debug!("Git repository already exists");
            return Ok(());
        }
        
        let output = std::process::Command::new("git")
            .arg("init")
            .current_dir(output_dir)
            .output();
            
        match output {
            Ok(output) if output.status.success() => {
                info!("Initialized git repository");
                
                // Create initial commit
                let _ = std::process::Command::new("git")
                    .args(&["add", "."])
                    .current_dir(output_dir)
                    .output();
                    
                let _ = std::process::Command::new("git")
                    .args(&["commit", "-m", "Initial commit from Aegis SDK"])
                    .current_dir(output_dir)
                    .output();
            }
            Ok(_) => debug!("Git init returned non-zero status"),
            Err(e) => debug!("Failed to initialize git repository: {}", e),
        }
        
        Ok(())
    }
    
    async fn setup_dev_environment(&self, output_dir: &Path) -> Result<()> {
        // Create VS Code settings if requested
        if self.config.development_tools.contains(&"linting-and-formatting".to_string()) {
            self.create_vscode_settings(output_dir).await?;
        }
        
        // Create GitHub Actions if requested
        if self.config.development_tools.contains(&"ci-cd-configuration".to_string()) {
            self.create_github_actions(output_dir).await?;
        }
        
        Ok(())
    }
    
    async fn create_vscode_settings(&self, output_dir: &Path) -> Result<()> {
        let vscode_dir = output_dir.join(".vscode");
        std::fs::create_dir_all(&vscode_dir)?;
        
        let settings = serde_json::json!({
            "rust-analyzer.cargo.features": "all",
            "rust-analyzer.checkOnSave.command": "clippy",
            "editor.formatOnSave": true,
            "files.exclude": {
                "target/": true,
                "Cargo.lock": true
            }
        });
        
        let settings_file = vscode_dir.join("settings.json");
        std::fs::write(settings_file, serde_json::to_string_pretty(&settings)?)?;
        
        // Create recommended extensions
        let extensions = serde_json::json!({
            "recommendations": [
                "rust-lang.rust-analyzer",
                "vadimcn.vscode-lldb",
                "serayuzgur.crates",
                "tamasfe.even-better-toml"
            ]
        });
        
        let extensions_file = vscode_dir.join("extensions.json");
        std::fs::write(extensions_file, serde_json::to_string_pretty(&extensions)?)?;
        
        debug!("Created VS Code settings");
        Ok(())
    }
    
    async fn create_github_actions(&self, output_dir: &Path) -> Result<()> {
        let workflows_dir = output_dir.join(".github/workflows");
        
        let ci_workflow = format!(r#"name: CI

on:
  push:
    branches: [ main, develop ]
  pull_request:
    branches: [ main ]

env:
  CARGO_TERM_COLOR: always

jobs:
  test:
    runs-on: ubuntu-latest
    
    steps:
    - uses: actions/checkout@v4
    
    - name: Install Rust
      uses: actions-rs/toolchain@v1
      with:
        toolchain: stable
        components: rustfmt, clippy
        override: true
        
    - name: Cache cargo dependencies
      uses: actions/cache@v3
      with:
        path: |
          ~/.cargo/registry
          ~/.cargo/git
          target
        key: ${{{{ runner.os }}}}-cargo-${{{{ hashFiles('**/Cargo.lock') }}}}
        
    - name: Check formatting
      run: cargo fmt --all -- --check
      
    - name: Lint with clippy
      run: cargo clippy --all-targets --all-features -- -D warnings
      
    - name: Run tests
      run: cargo test --all-features
      
    {}
    
  build:
    runs-on: ubuntu-latest
    needs: test
    
    steps:
    - uses: actions/checkout@v4
    
    - name: Install Rust
      uses: actions-rs/toolchain@v1
      with:
        toolchain: stable
        override: true
        
    - name: Build release
      run: cargo build --release --all-features
      
    - name: Upload artifact
      uses: actions/upload-artifact@v3
      with:
        name: {}-plugin
        path: target/release/lib{}.rlib
"#, 
            if self.config.development_tools.contains(&"code-coverage".to_string()) {
                r#"- name: Generate coverage report
      run: |
        cargo install cargo-tarpaulin
        cargo tarpaulin --out xml
        
    - name: Upload coverage to Codecov
      uses: codecov/codecov-action@v3"#
            } else {
                ""
            },
            self.config.name,
            self.config.name.replace("-", "_")
        );
        
        let ci_file = workflows_dir.join("ci.yml");
        std::fs::write(ci_file, ci_workflow)?;
        
        debug!("Created GitHub Actions CI workflow");
        Ok(())
    }
}