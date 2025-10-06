//! # Aegis Development SDK
//! 
//! Advanced developer toolkit for building Aegis-Assets plugins with:
//! - Interactive project scaffolding
//! - Hot-reload development workflow  
//! - Comprehensive testing framework
//! - Documentation generation
//! - Code signing and distribution tools

use clap::{Parser, Subcommand};
use anyhow::{Result, Context};
use colored::*;
use std::path::PathBuf;

mod commands;
mod templates;
mod scaffold;
mod testing;
mod docs;
mod hotreload;
mod signing;
mod marketplace;

use commands::*;

#[derive(Parser)]
#[command(name = "aegis-dev")]
#[command(about = "🛠️  Aegis-Assets Plugin Development SDK")]
#[command(version)]
#[command(long_about = "Advanced developer toolkit for building compliance-first asset extraction plugins")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    
    /// Enable verbose output
    #[arg(short, long, global = true)]
    verbose: bool,
    
    /// Working directory for operations
    #[arg(short, long, global = true)]
    dir: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Commands {
    /// 🚀 Create new plugin project with interactive setup
    New {
        /// Plugin name (e.g., "my-engine-plugin")
        name: String,
        
        /// Template to use (unity, unreal, custom, minimal)
        #[arg(short, long, default_value = "interactive")]
        template: String,
        
        /// Target engine (unity, unreal, other)
        #[arg(short, long)]
        engine: Option<String>,
        
        /// Skip interactive prompts and use defaults
        #[arg(long)]
        no_interactive: bool,
        
        /// Output directory
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    
    /// 🔄 Start hot-reload development server
    Dev {
        /// Plugin project directory
        #[arg(value_name = "PROJECT_DIR")]
        project: Option<PathBuf>,
        
        /// Port for development server
        #[arg(short, long, default_value = "3001")]
        port: u16,
        
        /// Watch additional directories
        #[arg(long)]
        watch: Vec<PathBuf>,
        
        /// Test file patterns to run on changes
        #[arg(long)]
        test_pattern: Option<String>,
    },
    
    /// 🧪 Testing and validation tools
    Test {
        #[command(subcommand)]
        command: TestCommands,
    },
    
    /// 📚 Documentation generation and management
    Docs {
        #[command(subcommand)]
        command: DocsCommands,
    },
    
    /// ✍️ Code signing and security tools
    Sign {
        #[command(subcommand)]
        command: SignCommands,
    },
    
    /// 🏪 Marketplace publishing and management
    Market {
        #[command(subcommand)]
        command: MarketCommands,
    },
    
    /// 🔧 Project configuration and utilities
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },
    
    /// 📊 Performance profiling and benchmarking
    Profile {
        /// Test files to profile
        #[arg(value_name = "TEST_FILES")]
        test_files: Vec<PathBuf>,
        
        /// Output format (json, html, text)
        #[arg(short, long, default_value = "html")]
        format: String,
        
        /// Number of iterations for benchmarks
        #[arg(short, long, default_value = "10")]
        iterations: usize,
    },
    
    /// 🎯 Plugin validation and compliance checking
    Validate {
        /// Plugin project or built plugin file
        #[arg(value_name = "PLUGIN_PATH")]
        plugin: PathBuf,
        
        /// Validation profile (basic, strict, enterprise)
        #[arg(short, long, default_value = "basic")]
        profile: String,
        
        /// Generate detailed compliance report
        #[arg(long)]
        report: bool,
        
        /// Auto-fix issues where possible
        #[arg(long)]
        fix: bool,
    },
}

#[derive(Subcommand)]
enum TestCommands {
    /// Run plugin tests
    Run {
        /// Test files pattern
        #[arg(short, long)]
        pattern: Option<String>,
        
        /// Run tests in parallel
        #[arg(short, long)]
        parallel: bool,
        
        /// Generate coverage report
        #[arg(long)]
        coverage: bool,
        
        /// Test with specific asset files
        #[arg(long)]
        with_assets: Vec<PathBuf>,
    },
    
    /// Generate test assets and fixtures
    Generate {
        /// Asset type (unity-scene, unreal-map, texture, etc.)
        #[arg(value_name = "ASSET_TYPE")]
        asset_type: String,
        
        /// Output directory for test assets
        #[arg(short, long, default_value = "test-assets")]
        output: PathBuf,
        
        /// Number of variations to generate
        #[arg(short, long, default_value = "5")]
        count: usize,
    },
    
    /// Integration testing with real game files
    Integration {
        /// Game assets directory
        #[arg(value_name = "ASSETS_DIR")]
        assets_dir: PathBuf,
        
        /// Expected results file
        #[arg(short, long)]
        expected: Option<PathBuf>,
        
        /// Update expected results based on current output
        #[arg(long)]
        update_expected: bool,
    },
}

#[derive(Subcommand)]
enum DocsCommands {
    /// Generate plugin documentation
    Build {
        /// Output directory
        #[arg(short, long, default_value = "docs")]
        output: PathBuf,
        
        /// Include private items in documentation
        #[arg(long)]
        include_private: bool,
        
        /// Generate interactive examples
        #[arg(long)]
        examples: bool,
    },
    
    /// Serve documentation with live reload
    Serve {
        /// Port to serve on
        #[arg(short, long, default_value = "3002")]
        port: u16,
        
        /// Open browser automatically
        #[arg(long)]
        open: bool,
    },
    
    /// Initialize documentation template
    Init {
        /// Documentation type (api, tutorial, guide)
        #[arg(value_name = "DOC_TYPE")]
        doc_type: String,
    },
}

#[derive(Subcommand)]
enum SignCommands {
    /// Sign plugin for distribution
    Plugin {
        /// Plugin file to sign
        #[arg(value_name = "PLUGIN_FILE")]
        plugin: PathBuf,
        
        /// Signing key file
        #[arg(short, long)]
        key: PathBuf,
        
        /// Output signed plugin
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    
    /// Generate signing keys
    Keygen {
        /// Output key file
        #[arg(short, long, default_value = "plugin-signing-key")]
        output: PathBuf,
        
        /// Key type (ed25519, rsa)
        #[arg(short, long, default_value = "ed25519")]
        key_type: String,
    },
    
    /// Verify plugin signature
    Verify {
        /// Signed plugin file
        #[arg(value_name = "PLUGIN_FILE")]
        plugin: PathBuf,
        
        /// Public key for verification
        #[arg(short, long)]
        pubkey: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum MarketCommands {
    /// Publish plugin to marketplace
    Publish {
        /// Plugin project directory
        #[arg(value_name = "PROJECT_DIR")]
        project: Option<PathBuf>,
        
        /// Version to publish
        #[arg(short, long)]
        version: Option<String>,
        
        /// Dry run - don't actually publish
        #[arg(long)]
        dry_run: bool,
        
        /// Skip validation checks
        #[arg(long)]
        skip_validation: bool,
    },
    
    /// Update plugin metadata
    Update {
        /// Plugin ID
        #[arg(value_name = "PLUGIN_ID")]
        plugin_id: String,
        
        /// Metadata file
        #[arg(short, long)]
        metadata: PathBuf,
    },
    
    /// Check publication status
    Status {
        /// Plugin ID
        #[arg(value_name = "PLUGIN_ID")]
        plugin_id: String,
    },
    
    /// Download plugin source for study
    Download {
        /// Plugin ID
        #[arg(value_name = "PLUGIN_ID")]
        plugin_id: String,
        
        /// Output directory
        #[arg(short, long, default_value = ".")]
        output: PathBuf,
        
        /// Download specific version
        #[arg(short, long)]
        version: Option<String>,
    },
}

#[derive(Subcommand)]
enum ConfigCommands {
    /// Initialize configuration files
    Init {
        /// Configuration type (project, user, global)
        #[arg(value_name = "CONFIG_TYPE")]
        config_type: String,
    },
    
    /// Show current configuration
    Show,
    
    /// Set configuration value
    Set {
        /// Configuration key
        #[arg(value_name = "KEY")]
        key: String,
        
        /// Configuration value
        #[arg(value_name = "VALUE")]
        value: String,
    },
    
    /// Get configuration value
    Get {
        /// Configuration key
        #[arg(value_name = "KEY")]
        key: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    
    // Initialize logging
    init_logging(cli.verbose);
    
    // Change to working directory if specified
    if let Some(dir) = &cli.dir {
        std::env::set_current_dir(dir)
            .with_context(|| format!("Failed to change to directory: {}", dir.display()))?;
    }
    
    // Print banner
    print_banner();
    
    match cli.command {
        Commands::New { name, template, engine, no_interactive, output } => {
            handle_new_project(name, template, engine, no_interactive, output).await
        }
        Commands::Dev { project, port, watch, test_pattern } => {
            handle_dev_server(project, port, watch, test_pattern).await
        }
        Commands::Test { command } => {
            handle_test_commands(command).await
        }
        Commands::Docs { command } => {
            handle_docs_commands(command).await
        }
        Commands::Sign { command } => {
            handle_sign_commands(command).await
        }
        Commands::Market { command } => {
            handle_market_commands(command).await
        }
        Commands::Config { command } => {
            handle_config_commands(command).await
        }
        Commands::Profile { test_files, format, iterations } => {
            handle_profile(test_files, format, iterations).await
        }
        Commands::Validate { plugin, profile, report, fix } => {
            handle_validate(plugin, profile, report, fix).await
        }
    }
}

fn init_logging(verbose: bool) {
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
    
    let filter = if verbose {
        EnvFilter::from_default_env().add_directive("aegis_dev_sdk=debug".parse().unwrap())
    } else {
        EnvFilter::from_default_env().add_directive("aegis_dev_sdk=info".parse().unwrap())
    };
    
    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .init();
}

fn print_banner() {
    println!("{}", "🛠️  Aegis-Assets Plugin Development SDK".bold().cyan());
    println!("{}", "   Building the future of compliance-first asset extraction".dimmed());
    println!();
}