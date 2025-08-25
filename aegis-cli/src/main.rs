use anyhow::{Result, Context};
use clap::{Parser, Subcommand, ValueEnum};
use colored::*;
use std::path::{Path, PathBuf};
use aegis_core::{AegisCore, Config, ComplianceLevel};

mod commands;
mod ui;

use commands::{extract::ExtractCommand, recipe::RecipeCommand, compliance::ComplianceCommand};
use ui::{progress_bar, success, warning, error, info};

/// Aegis-Assets CLI - Compliance-first game asset extraction
#[derive(Parser)]
#[command(
    name = "aegis",
    version = env!("CARGO_PKG_VERSION"),
    about = "Compliance-first platform for game asset extraction, preservation, and creative workflows",
    long_about = None,
    arg_required_else_help = true
)]
struct Cli {
    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Disable colored output
    #[arg(long, global = true)]
    no_color: bool,

    /// Path to compliance profiles directory
    #[arg(long, global = true)]
    compliance_profiles: Option<PathBuf>,

    /// Enable strict compliance mode (blocks high-risk operations)
    #[arg(long, global = true)]
    strict: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Extract assets from game files
    Extract(ExtractCommand),
    
    /// Work with patch recipes
    Recipe(RecipeCommand),
    
    /// Check compliance status and manage profiles
    Compliance(ComplianceCommand),
    
    /// Show system information and plugin status
    Info {
        /// Show detailed plugin information
        #[arg(long)]
        plugins: bool,
        
        /// Show compliance profiles
        #[arg(long)]
        profiles: bool,
    },
    
    /// Test file format detection
    Detect {
        /// File to analyze
        file: PathBuf,
        
        /// Show detailed analysis
        #[arg(long)]
        detailed: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    
    // Initialize colored output
    colored::control::set_override(!cli.no_color);
    
    // Initialize logging
    init_logging(cli.verbose)?;
    
    // Initialize Aegis-Core
    let mut core = initialize_core(&cli)?;
    
    // Execute command
    match &cli.command {
        Commands::Extract(cmd) => cmd.execute(&core),
        Commands::Recipe(cmd) => cmd.execute(&core),
        Commands::Compliance(cmd) => cmd.execute(&core),
        Commands::Info { plugins, profiles } => {
            show_system_info(&core, *plugins, *profiles)
        }
        Commands::Detect { file, detailed } => {
            detect_format(&core, file, *detailed)
        }
    }
}

fn init_logging(verbose: bool) -> Result<()> {
    let level = if verbose { "debug" } else { "info" };
    
    tracing_subscriber::fmt()
        .with_env_filter(format!("aegis_core={},aegis_cli={}", level, level))
        .with_target(false)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .init();
    
    Ok(())
}

fn initialize_core(cli: &Cli) -> Result<AegisCore> {
    let config = Config {
        max_memory_mb: 4096,
        enable_parallel: true,
        temp_dir: None,
        enable_ai_features: false,
        enterprise_config: None,
    };
    
    let mut core = AegisCore::with_config(config)
        .context("Failed to initialize Aegis-Core")?;
    
    // Load compliance profiles
    if let Some(profiles_dir) = &cli.compliance_profiles {
        info(&format!("Loading compliance profiles from {}", profiles_dir.display()));
        core.load_compliance_profiles(profiles_dir)
            .context("Failed to load compliance profiles")?;
    } else {
        // Try to load from default location
        let default_profiles = Path::new("compliance-profiles");
        if default_profiles.exists() {
            core.load_compliance_profiles(default_profiles)
                .context("Failed to load compliance profiles")?;
        }
    }
    
    // Register plugins
    register_plugins(&mut core)?;
    
    Ok(core)
}

fn register_plugins(core: &mut AegisCore) -> Result<()> {
    // Register Unity plugin
    #[cfg(feature = "unity")]
    {
        use aegis_unity_plugin::UnityPluginFactory;
        core.register_plugin(Box::new(UnityPluginFactory));
    }
    
    // Register Unreal plugin when implemented
    #[cfg(feature = "unreal")]
    {
        // core.register_plugin(Box::new(UnrealPluginFactory));
        info("Unreal plugin not yet implemented");
    }
    
    Ok(())
}

fn show_system_info(core: &AegisCore, show_plugins: bool, show_profiles: bool) -> Result<()> {
    let info = core.system_info();
    
    println!("{}", "Aegis-Assets System Information".bright_blue().bold());
    println!("Version: {}", info.version.bright_green());
    println!("Git Hash: {}", info.git_hash);
    println!("Registered Plugins: {}", info.registered_plugins);
    println!("Compliance Profiles: {}", info.compliance_profiles);
    
    if show_plugins {
        println!("\n{}", "Registered Plugins:".bright_yellow().bold());
        if info.registered_plugins == 0 {
            println!("  {}", "No plugins registered".dimmed());
        } else {
            // This would show actual plugin info in a real implementation
            println!("  {} Unity Plugin v{}", "✓".bright_green(), env!("CARGO_PKG_VERSION"));
        }
    }
    
    if show_profiles {
        println!("\n{}", "Compliance Profiles:".bright_yellow().bold());
        if info.compliance_profiles == 0 {
            println!("  {}", "No compliance profiles loaded".dimmed());
            println!("  {}", "Use --compliance-profiles to specify directory".dimmed());
        } else {
            println!("  {} compliance profiles loaded", info.compliance_profiles);
        }
    }
    
    println!("\n{}", "Configuration:".bright_yellow().bold());
    println!("  Max Memory: {} MB", info.config.max_memory_mb);
    println!("  Parallel Processing: {}", if info.config.enable_parallel { "✓".bright_green() } else { "✗".red() });
    println!("  AI Features: {}", if info.config.enable_ai_features { "✓".bright_green() } else { "✗".dimmed() });
    println!("  Enterprise Mode: {}", if info.config.enterprise_config.is_some() { "✓".bright_green() } else { "✗".dimmed() });
    
    Ok(())
}

fn detect_format(core: &AegisCore, file_path: &Path, detailed: bool) -> Result<()> {
    if !file_path.exists() {
        return Err(anyhow::anyhow!("File does not exist: {}", file_path.display()));
    }
    
    info(&format!("Analyzing file: {}", file_path.display()));
    
    // Read file header
    let header = std::fs::read(file_path.parent().unwrap_or(file_path))
        .context("Failed to read file")?;
    
    let sample = if header.len() > 1024 { &header[..1024] } else { &header };
    
    println!("{}", "Format Detection Results:".bright_blue().bold());
    
    // Check Unity format
    if aegis_unity_plugin::UnityArchive::detect(sample) {
        success("Unity format detected");
        
        if detailed {
            println!("  Format: Unity Asset Bundle or Serialized File");
            if sample.starts_with(b"UnityFS\0") {
                println!("  Type: UnityFS (Unity 5.3+)");
            } else if sample.starts_with(b"UnityRaw") {
                println!("  Type: UnityRaw (Legacy)");
            } else {
                println!("  Type: Serialized File");
            }
        }
    } else {
        warning("No supported format detected");
        
        if detailed {
            println!("  File size: {} bytes", header.len());
            println!("  Header (first 16 bytes): {:02x?}", &sample[..sample.len().min(16)]);
            
            // Check for common signatures
            if sample.starts_with(b"PK") {
                info("Possible ZIP archive");
            } else if sample.starts_with(b"\x7fELF") {
                info("Possible ELF binary");
            } else if sample.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
                info("PNG image detected");
            }
        }
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;
    
    #[test]
    fn verify_cli() {
        Cli::command().debug_assert();
    }
}
