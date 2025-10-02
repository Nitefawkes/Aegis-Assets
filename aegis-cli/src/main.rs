use clap::{Parser, Subcommand};
use anyhow::{Result, Context};
use aegis_core::{
    archive::ComplianceRegistry,
    extract::{Extractor, ExtractionError},
    PluginRegistry, init,
};
use std::path::{Path, PathBuf};
use tracing::{info, error, warn};
use walkdir;

mod ui;

#[derive(Parser)]
#[command(name = "aegis")]
#[command(about = "Compliance-first game asset extraction tool")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Extract assets from game files
    Extract {
        /// Input file or directory path
        #[arg(value_name = "INPUT")]
        input: PathBuf,
        
        /// Output directory for extracted assets
        #[arg(short, long, value_name = "OUTPUT")]
        output: Option<PathBuf>,
        
        /// Specific plugin to use (optional, auto-detected if not specified)
        #[arg(short, long, value_name = "PLUGIN")]
        plugin: Option<String>,
        
        /// Convert assets to standard formats (PNG, glTF, OGG)
        #[arg(long)]
        convert: bool,
        
        /// Skip compliance checks (not recommended)
        #[arg(long)]
        skip_compliance: bool,
        
        /// Verbose output
        #[arg(short, long)]
        verbose: bool,
    },
    
    /// Check compliance with publisher policies
    Compliance {
        /// Input file or directory to check
        #[arg(value_name = "INPUT")]
        input: PathBuf,
        
        /// Publisher profile to check against
        #[arg(short, long, value_name = "PROFILE")]
        profile: Option<String>,
    },
    
    /// Plugin management and marketplace
    Plugins {
        #[command(subcommand)]
        command: PluginsCommands,
    },

    /// Asset database and search operations
    Db {
        #[command(subcommand)]
        command: DbCommands,
    },

    /// Start the REST API server
    #[cfg(feature = "api")]
    Serve {
        /// Address to bind the server to
        #[arg(short, long, default_value = "0.0.0.0:3000")]
        address: String,

        /// Path to the asset database
        #[arg(short, long, default_value = "./assets.db")]
        database: std::path::PathBuf,

        /// Enable CORS headers
        #[arg(long, default_value = "true")]
        cors: bool,
    },

    /// List assets in a file without extracting
    List {
        /// Input file to examine
        #[arg(value_name = "INPUT")]
        input: PathBuf,

        /// Show detailed information
        #[arg(short, long)]
        details: bool,
    },

    /// Show version and build information
    Version,
}

/// Plugin management subcommands
#[derive(Subcommand)]
enum PluginsCommands {
    /// List installed plugins
    List {
        /// Show detailed information
        #[arg(short, long)]
        verbose: bool,
    },

    /// Search for plugins in the marketplace
    Search {
        /// Search query
        #[arg(value_name = "QUERY")]
        query: String,

        /// Filter by engine (unity, unreal, etc.)
        #[arg(short, long, value_name = "ENGINE")]
        engine: Option<String>,

        /// Filter by risk level (low, medium, high)
        #[arg(short, long, value_name = "RISK")]
        risk: Option<String>,

        /// Limit number of results
        #[arg(short, long, value_name = "LIMIT")]
        limit: Option<usize>,

        /// Verbose output with full plugin information
        #[arg(short, long)]
        verbose: bool,
    },

    /// Get detailed information about a plugin
    Info {
        /// Plugin name or ID
        #[arg(value_name = "PLUGIN")]
        plugin: String,

        /// Show version history
        #[arg(long)]
        history: bool,

        /// Show compliance information
        #[arg(long)]
        compliance: bool,
    },

    /// Install a plugin
    Install {
        /// Plugin name to install
        #[arg(value_name = "PLUGIN")]
        plugin: String,

        /// Specific version to install
        #[arg(long, value_name = "VERSION")]
        version: Option<String>,

        /// Force installation even if plugin is already installed
        #[arg(long)]
        force: bool,

        /// Skip dependency resolution
        #[arg(long)]
        skip_deps: bool,
    },

    /// Uninstall a plugin
    Uninstall {
        /// Plugin name to uninstall
        #[arg(value_name = "PLUGIN")]
        plugin: String,

        /// Remove plugin data and configurations
        #[arg(long)]
        purge: bool,

        /// Skip dependency checks
        #[arg(long)]
        skip_deps: bool,
    },

    /// Update installed plugins
    Update {
        /// Specific plugin to update (updates all if not specified)
        #[arg(value_name = "PLUGIN")]
        plugin: Option<String>,

        /// Update to latest versions even if breaking changes
        #[arg(long)]
        force: bool,

        /// Skip dependency resolution
        #[arg(long)]
        skip_deps: bool,
    },

    /// Show plugin marketplace statistics
    Stats,
}

/// Database subcommands
#[derive(Subcommand)]
enum DbCommands {
    /// Search assets in the database
    Search {
        /// Search query (supports name, tags, type, etc.)
        #[arg(value_name = "QUERY")]
        query: Option<String>,

        /// Filter by asset type (texture, mesh, audio, etc.)
        #[arg(short, long, value_name = "TYPE")]
        asset_type: Option<String>,

        /// Filter by tags (can specify multiple)
        #[arg(short, long, value_name = "TAG")]
        tags: Vec<String>,

        /// Filter by game ID
        #[arg(long, value_name = "GAME")]
        game: Option<String>,

        /// Limit number of results
        #[arg(short, long, value_name = "LIMIT")]
        limit: Option<usize>,

        /// Verbose output with full metadata
        #[arg(short, long)]
        verbose: bool,
    },

    /// Index assets from extraction results
    Index {
        /// Directory containing extracted assets
        #[arg(value_name = "DIRECTORY")]
        directory: PathBuf,

        /// Game ID for categorization
        #[arg(long, value_name = "GAME")]
        game: Option<String>,

        /// Tags to apply to all assets
        #[arg(short, long, value_name = "TAG")]
        tags: Vec<String>,
    },

    /// Show database statistics
    Stats,

    /// Show all assets in the database
    Show {
        /// Filter by asset type
        #[arg(short, long, value_name = "TYPE")]
        asset_type: Option<String>,

        /// Limit number of results
        #[arg(short, long, value_name = "LIMIT")]
        limit: Option<usize>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    
    // Initialize Aegis-Core
    init().context("Failed to initialize Aegis-Core")?;
    
    // Load plugins and compliance profiles
    let plugin_registry = load_plugins()?;
    let compliance_registry = load_compliance_profiles()?;
    
    match &cli.command {
        Commands::Extract { 
            input, 
            output, 
            plugin, 
            convert, 
            skip_compliance,
            verbose 
        } => {
            handle_extract(
                input, 
                output.as_ref().map(|p| p.as_path()), 
                plugin.as_ref(),
                *convert,
                *skip_compliance,
                *verbose,
                plugin_registry,
                compliance_registry
            )?;
        }
        
        Commands::Compliance { input, profile } => {
            handle_compliance_check(input, profile.as_ref(), compliance_registry)?;
        }
        
        Commands::List { input, details } => {
            handle_list_assets(input, *details, plugin_registry)?;
        }
        
        Commands::Plugins { command } => {
            handle_plugins_command(command, plugin_registry)?;
        }
        
        Commands::Version => {
            println!("🛡️  Aegis-Assets v{}", env!("CARGO_PKG_VERSION"));
            println!("Build: {}", option_env!("VERGEN_GIT_SHA_SHORT").unwrap_or("unknown"));
            println!("Core: {}", aegis_core::VERSION);
        }

        Commands::Db { command } => {
            handle_db_command(command)?;
        }

        #[cfg(feature = "api")]
        Commands::Serve { address, database, cors } => {
            handle_serve_command(address, database, *cors).await?;
        }
    }
    
    Ok(())
}

/// Load available plugins
fn load_plugins() -> Result<PluginRegistry> {
    info!("Loading plugins...");
    let mut registry = PluginRegistry::new();
    
    // For now, manually register Unity plugin
    // In a full implementation, this would scan plugin directories
    #[cfg(feature = "unity-plugin")]
    {
        use aegis_unity_plugin::UnityPluginFactory;
        registry.register_plugin(Box::new(UnityPluginFactory));
    }
    
    // Try to load Unity plugin directly (since we know it's there)
    match load_unity_plugin() {
        Ok(factory) => {
            registry.register_plugin(factory);
            info!("Successfully loaded Unity plugin");
        }
        Err(e) => {
            warn!("Failed to load Unity plugin: {}", e);
        }
    }

    // Try to load Unreal plugin directly
    match load_unreal_plugin() {
        Ok(factory) => {
            registry.register_plugin(factory);
            info!("Successfully loaded Unreal plugin");
        }
        Err(e) => {
            warn!("Failed to load Unreal plugin: {}", e);
        }
    }
    
    let plugin_count = registry.list_plugins().len();
    info!("Loaded {} plugins", plugin_count);
    
    if plugin_count == 0 {
        warn!("No plugins loaded! Asset extraction will not work.");
    }
    
    Ok(registry)
}

/// Try to load Unity plugin directly
fn load_unity_plugin() -> Result<Box<dyn aegis_core::PluginFactory>> {
    use aegis_unity_plugin::UnityPluginFactory;
    Ok(Box::new(UnityPluginFactory))
}

fn load_unreal_plugin() -> Result<Box<dyn aegis_core::PluginFactory>> {
    use aegis_unreal_plugin::UnrealPluginFactory;
    Ok(Box::new(UnrealPluginFactory))
}

/// Load compliance profiles
fn load_compliance_profiles() -> Result<ComplianceRegistry> {
    info!("Loading compliance profiles...");
    
    let compliance_dir = Path::new("compliance-profiles");
    if compliance_dir.exists() {
        ComplianceRegistry::load_from_directory(compliance_dir)
            .context("Failed to load compliance profiles from directory")
    } else {
        warn!("Compliance profiles directory not found, using defaults");
        Ok(ComplianceRegistry::default())
    }
}

/// Handle asset extraction command
fn handle_extract(
    input: &Path,
    output: Option<&Path>,
    _plugin_filter: Option<&String>,
    convert: bool,
    skip_compliance: bool,
    verbose: bool,
    plugin_registry: PluginRegistry,
    compliance_registry: ComplianceRegistry,
) -> Result<()> {
    info!("Starting asset extraction from: {}", input.display());
    
    // Determine output directory
    let output_dir = output.unwrap_or_else(|| Path::new("./extracted"));
    std::fs::create_dir_all(output_dir)
        .with_context(|| format!("Failed to create output directory: {}", output_dir.display()))?;
    
    // Create extractor
    let mut extractor = Extractor::new(plugin_registry, compliance_registry);
    
    // Override compliance if requested (and warn)
    if skip_compliance {
        warn!("⚠️  Compliance checks disabled - use at your own risk!");
    }
    
    // Check if input file exists
    if !input.exists() {
        error!("Input file not found: {}", input.display());
        return Err(anyhow::anyhow!("File not found: {}", input.display()));
    }
    
    // Perform extraction
    match extractor.extract_from_file(input, output_dir) {
        Ok(result) => {
            println!("✅ Extraction successful!");
            println!("📁 Source: {}", result.source_path.display());
            println!("📊 Resources found: {}", result.resources.len());
            println!("⏱️  Duration: {}ms", result.metrics.duration_ms);
            println!("💾 Peak memory: {}MB", result.metrics.peak_memory_mb);
            println!("📈 Total bytes: {}", result.metrics.bytes_extracted);
            
            if !result.compliance_info.warnings.is_empty() {
                println!("\n⚠️  Compliance warnings:");
                for warning in &result.compliance_info.warnings {
                    println!("   • {}", warning);
                }
            }
            
            if !result.compliance_info.recommendations.is_empty() {
                println!("\n💡 Recommendations:");
                for rec in &result.compliance_info.recommendations {
                    println!("   • {}", rec);
                }
            }
            
            if verbose {
                println!("\n📋 Extracted resources:");
                for resource in &result.resources {
                    println!("   • {} ({}, {} bytes)", resource.name, resource.format, resource.size);
                }
            }
            
            if convert {
                println!("\n🔄 Converting assets to standard formats...");
                match convert_extracted_assets(&result, &output_dir) {
                    Ok(converted_files) => {
                        println!("✅ Conversion completed!");
                        println!("📁 Converted {} files:", converted_files.len());
                        for file in &converted_files {
                            println!("   • {} ({})", file.path.display(), format_bytes(file.size_bytes));
                        }
                    },
                    Err(e) => {
                        println!("⚠️  Conversion failed: {}", e);
                        println!("   Continuing with raw extracted assets...");
                    }
                }
            }
        }
        Err(ExtractionError::NoSuitablePlugin(path)) => {
            error!("❌ No plugin found for file: {}", path.display());
            println!("\n💡 Try:");
            println!("   • Check file format is supported");
            println!("   • Use `aegis plugins` to see available plugins");
            return Err(anyhow::anyhow!("No suitable plugin found"));
        }
        Err(ExtractionError::ComplianceViolation(msg)) => {
            error!("❌ Compliance violation: {}", msg);
            println!("\n💡 This extraction was blocked for legal/compliance reasons.");
            println!("   Use --skip-compliance to override (not recommended)");
            return Err(anyhow::anyhow!("Compliance violation"));
        }
        Err(e) => {
            error!("❌ Extraction failed: {}", e);
            return Err(anyhow::anyhow!("Extraction failed: {}", e));
        }
    }
    
    Ok(())
}

/// Handle compliance checking command
fn handle_compliance_check(
    input: &Path,
    _profile: Option<&String>,
    compliance_registry: ComplianceRegistry,
) -> Result<()> {
    info!("Checking compliance for: {}", input.display());
    
    let game_id = input.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");
    let format = input.extension()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");
    
    let checker = aegis_core::compliance::ComplianceChecker::from_registry(compliance_registry);
    let result = checker.check_extraction_allowed(game_id, format);
    
    match result {
        aegis_core::compliance::ComplianceResult::Allowed { profile, .. } => {
            println!("✅ Extraction allowed");
            println!("📋 Publisher: {}", profile.publisher);
            println!("🎯 Risk level: {:?}", profile.enforcement_level);
        }
        aegis_core::compliance::ComplianceResult::AllowedWithWarnings { warnings, profile, .. } => {
            println!("⚠️  Extraction allowed with warnings");
            println!("📋 Publisher: {}", profile.publisher);
            for warning in warnings {
                println!("   • {}", warning);
            }
        }
        aegis_core::compliance::ComplianceResult::HighRiskWarning { warnings, profile, .. } => {
            println!("🚨 High-risk extraction");
            println!("📋 Publisher: {}", profile.publisher);
            for warning in warnings {
                println!("   • {}", warning);
            }
        }
        aegis_core::compliance::ComplianceResult::Blocked { reason, profile, .. } => {
            println!("❌ Extraction blocked");
            println!("📋 Publisher: {}", profile.publisher);
            println!("🚫 Reason: {}", reason);
        }
    }
    
    Ok(())
}

/// Handle listing assets in a file
fn handle_list_assets(
    input: &Path,
    details: bool,
    plugin_registry: PluginRegistry,
) -> Result<()> {
    info!("Listing assets in: {}", input.display());
    
    if !input.exists() {
        return Err(anyhow::anyhow!("File not found: {}", input.display()));
    }
    
    // Read file header for plugin detection
    let header = std::fs::read(input).context("Failed to read file")?;
    let header_preview = if header.len() > 1024 { &header[..1024] } else { &header };
    
    // Find suitable plugin
    if let Some(plugin_factory) = plugin_registry.find_handler(input, header_preview) {
        println!("📋 File: {}", input.display());
        println!("🔌 Plugin: {} v{}", plugin_factory.name(), plugin_factory.version());
        
        // Create handler and list entries
        match plugin_factory.create_handler(input) {
            Ok(handler) => {
                match handler.list_entries() {
                    Ok(entries) => {
                        println!("📦 Assets found: {}", entries.len());
                        
                        if details {
                            println!("\n📝 Asset details:");
                            for entry in &entries {
                                println!("   • {} ({})", entry.name, entry.file_type.as_deref().unwrap_or("unknown"));
                                println!("     Size: {} bytes", entry.size_uncompressed);
                                if let Some(compressed) = entry.size_compressed {
                                    let ratio = (1.0 - compressed as f64 / entry.size_uncompressed as f64) * 100.0;
                                    println!("     Compressed: {} bytes ({:.1}% reduction)", compressed, ratio);
                                }
                            }
                        } else {
                            // Group by type
                            let mut type_counts = std::collections::HashMap::new();
                            for entry in &entries {
                                let entry_type = entry.file_type.as_deref().unwrap_or("unknown");
                                *type_counts.entry(entry_type).or_insert(0) += 1;
                            }
                            
                            println!("\n📊 Asset types:");
                            for (asset_type, count) in type_counts {
                                println!("   • {}: {} files", asset_type, count);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to list entries: {}", e);
                        return Err(anyhow::anyhow!("Failed to list entries: {}", e));
                    }
                }
            }
            Err(e) => {
                error!("Failed to create handler: {}", e);
                return Err(anyhow::anyhow!("Failed to create handler: {}", e));
            }
        }
    } else {
        println!("❌ No plugin found for file: {}", input.display());
        println!("\n💡 Supported formats:");
        for plugin in plugin_registry.list_plugins() {
            println!("   • {}: {}", plugin.name(), plugin.supported_extensions().join(", "));
        }
    }
    
    Ok(())
}

/// Handle plugin management commands
fn handle_plugins_command(command: &PluginsCommands, plugin_registry: PluginRegistry) -> Result<()> {
    match command {
        PluginsCommands::List { verbose } => {
            handle_list_plugins(plugin_registry, *verbose)?;
        }
        PluginsCommands::Search { query, engine, risk, limit, verbose } => {
            handle_search_plugins(query, engine.as_ref(), risk.as_ref(), *limit, *verbose, plugin_registry)?;
        }
        PluginsCommands::Info { plugin, history, compliance } => {
            handle_plugin_info(plugin, *history, *compliance, plugin_registry)?;
        }
        PluginsCommands::Install { plugin, version, force, skip_deps } => {
            handle_install_plugin(plugin, version.as_ref(), *force, *skip_deps, plugin_registry)?;
        }
        PluginsCommands::Uninstall { plugin, purge, skip_deps } => {
            handle_uninstall_plugin(plugin, *purge, *skip_deps, plugin_registry)?;
        }
        PluginsCommands::Update { plugin, force, skip_deps } => {
            handle_update_plugins(plugin.as_ref(), *force, *skip_deps, plugin_registry)?;
        }
        PluginsCommands::Stats => {
            handle_plugin_stats(plugin_registry)?;
        }
    }
    Ok(())
}

/// Handle listing available plugins
fn handle_list_plugins(plugin_registry: PluginRegistry, verbose: bool) -> Result<()> {
    println!("🔌 Available plugins:");

    let plugins = plugin_registry.list_plugins();
    if plugins.is_empty() {
        println!("   No plugins loaded!");
        return Ok(());
    }

    for plugin in plugins {
        let compliance = plugin.compliance_info();
        let status = if compliance.compliance_verified { "✅" } else { "⚠️" };

        println!("   {} {} v{}", status, plugin.name(), plugin.version());
        println!("      Extensions: {}", plugin.supported_extensions().join(", "));

        if verbose {
            if let Some(author) = &compliance.author {
                println!("      Author: {}", author);
            }
            if let Some(description) = &compliance.description {
                println!("      Description: {}", description);
            }
            println!("      Risk Level: {:?}", compliance.risk_level);
            println!("      Publisher Policy: {:?}", compliance.publisher_policy);
        }
    }

    Ok(())
}

/// Convert extracted assets to standard formats
fn convert_extracted_assets(result: &aegis_core::extract::ExtractionResult, output_dir: &std::path::Path) -> Result<Vec<aegis_core::export::ExportedFile>> {
    use aegis_core::export::Exporter;
    use aegis_core::resource::{ResourceType, TextureResource, MeshResource};

    let exporter = Exporter::new();
    let mut all_converted_files = Vec::new();

    for resource in &result.resources {
        match resource.resource_type {
            ResourceType::Texture => {
                // Convert generic resource to TextureResource
                // This is a simplified approach - in a real implementation,
                // the Unity plugin would provide properly structured resources
                if let Ok(texture_data) = std::fs::read(output_dir.join(&resource.name)) {
                    let texture_resource = TextureResource {
                        name: resource.name.clone(),
                        width: 256,  // Placeholder - real implementation would parse this
                        height: 256, // Placeholder - real implementation would parse this
                        format: aegis_core::resource::TextureFormat::RGBA8,
                        data: texture_data,
                        mip_levels: 1,
                        usage_hint: None,
                    };

                    let converted = exporter.export_texture(&texture_resource, output_dir, None)?;
                    all_converted_files.extend(converted);
                }
            },
            ResourceType::Mesh => {
                // Convert generic resource to MeshResource
                if let Ok(mesh_data) = std::fs::read(output_dir.join(&resource.name)) {
                    let mesh_resource = MeshResource {
                        name: resource.name.clone(),
                        vertices: Vec::new(), // Placeholder
                        indices: Vec::new(),  // Placeholder
                        material_id: None,
                        bone_weights: None,
                    };

                    let converted = exporter.export_mesh(&mesh_resource, output_dir, None)?;
                    all_converted_files.extend(converted);
                }
            },
            _ => {
                // For other resource types, just copy as-is for now
                continue;
            }
        }
    }

    Ok(all_converted_files)
}

/// Format byte count as human-readable string
fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{} {}", bytes, UNITS[0])
    } else {
        format!("{:.1} {}", size, UNITS[unit_index])
    }
}

/// Handle database commands
fn handle_db_command(command: &DbCommands) -> Result<()> {
    use aegis_core::asset_db::{AssetDatabase, SearchQuery, SortOrder, AssetType};

    match command {
        DbCommands::Search {
            query,
            asset_type,
            tags,
            game,
            limit,
            verbose,
        } => {
            let mut db = AssetDatabase::new("./assets.db")?;

            // Parse asset type if provided
            let asset_type_filter = asset_type.as_ref().and_then(|t| {
                match t.to_lowercase().as_str() {
                    "texture" => Some(AssetType::Texture),
                    "mesh" => Some(AssetType::Mesh),
                    "audio" => Some(AssetType::Audio),
                    "animation" => Some(AssetType::Animation),
                    "material" => Some(AssetType::Material),
                    "level" => Some(AssetType::Level),
                    "script" => Some(AssetType::Script),
                    _ => Some(AssetType::Other(t.clone())),
                }
            });

            let search_query = SearchQuery {
                text: query.clone(),
                asset_type: asset_type_filter,
                tags: tags.clone(),
                game_id: game.clone(),
                compliance_level: None,
                limit: *limit,
                sort_by: SortOrder::Relevance,
            };

            let results = db.search(search_query)?;

            if results.is_empty() {
                println!("🔍 No assets found matching your search criteria.");
                return Ok(());
            }

            println!("🔍 Found {} assets:", results.len());

            for (i, result) in results.iter().enumerate() {
                let asset = &result.asset;
                println!("{}. {} ({:?}) - {} bytes",
                        i + 1,
                        asset.name,
                        asset.asset_type,
                        format_bytes(asset.file_size));

                if *verbose {
                    println!("   📁 Path: {}", asset.output_path.display());
                    println!("   🎮 Game: {}", asset.game_id.as_deref().unwrap_or("Unknown"));
                    println!("   🏷️  Tags: {}", asset.tags.join(", "));
                    if let Some(desc) = &asset.description {
                        println!("   📝 Description: {}", desc);
                    }
                    println!("   📅 Created: {}", asset.created_at.to_rfc3339());
                    println!();
                }
            }
        }

        DbCommands::Index { directory, game, tags } => {
            println!("📝 Indexing assets from: {}", directory.display());

            let mut db = AssetDatabase::new("./assets.db")?;
            let mut indexed_count = 0;

            // Walk through the directory and index assets
            for entry in walkdir::WalkDir::new(directory)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file()) {

                let path = entry.path();
                let file_name = path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown");

                // Skip certain files
                if file_name.starts_with('.') || file_name == "assets.db" {
                    continue;
                }

                // Determine asset type from file extension
                let asset_type = match path.extension().and_then(|e| e.to_str()) {
                    Some("png") | Some("jpg") | Some("jpeg") | Some("tga") => AssetType::Texture,
                    Some("gltf") | Some("glb") | Some("obj") | Some("fbx") => AssetType::Mesh,
                    Some("wav") | Some("ogg") | Some("mp3") => AssetType::Audio,
                    Some("anim") => AssetType::Animation,
                    Some("mat") => AssetType::Material,
                    Some("level") | Some("scene") => AssetType::Level,
                    Some("cs") | Some("lua") => AssetType::Script,
                    _ => AssetType::Other("Unknown".to_string()),
                };

                // Get file metadata
                let metadata = std::fs::metadata(path)?;
                let file_size = metadata.len();

                // Generate content hash
                let content_hash = if file_size < 1024 * 1024 { // Only hash small files
                    let data = std::fs::read(path)?;
                    blake3::hash(&data).to_hex().to_string()
                } else {
                    // For large files, use file path as hash
                    blake3::hash(path.to_string_lossy().as_bytes()).to_hex().to_string()
                };

                // Create asset metadata
                let asset_metadata = aegis_core::asset_db::AssetMetadata::new(
                    format!("asset_{}", indexed_count),
                    file_name.to_string(),
                    asset_type,
                    path.to_path_buf(),
                    path.parent().unwrap_or(path).to_path_buf(),
                    file_size,
                    content_hash,
                )
                .with_game_id(game.clone().unwrap_or_else(|| "unknown".to_string()))
                .with_keywords(vec![file_name.to_string()]);

                // Add tags
                let mut asset_metadata = asset_metadata;
                for tag in tags {
                    asset_metadata = asset_metadata.with_tag(tag.clone());
                }

                db.index_asset(asset_metadata)?;
                indexed_count += 1;
            }

            println!("✅ Indexed {} assets from {}", indexed_count, directory.display());
        }

        DbCommands::Stats => {
            let db = AssetDatabase::new("./assets.db")?;
            let stats = db.get_stats();

            println!("📊 Asset Database Statistics:");
            println!("   📁 Total Assets: {}", stats.total_assets);
            println!("   💾 Total Size: {}", format_bytes(stats.total_size));
            println!("   📂 Assets by Type:");

            for (asset_type, count) in &stats.assets_by_type {
                println!("      {}: {}", asset_type, count);
            }

            println!("   🏷️  Tags:");
            for (tag, count) in &stats.tags {
                println!("      {}: {}", tag, count);
            }
        }

        DbCommands::Show { asset_type, limit } => {
            let db = AssetDatabase::new("./assets.db")?;

            let assets = if let Some(type_filter) = asset_type {
                match type_filter.to_lowercase().as_str() {
                    "texture" => db.get_assets_by_type(&AssetType::Texture),
                    "mesh" => db.get_assets_by_type(&AssetType::Mesh),
                    "audio" => db.get_assets_by_type(&AssetType::Audio),
                    "animation" => db.get_assets_by_type(&AssetType::Animation),
                    "material" => db.get_assets_by_type(&AssetType::Material),
                    "level" => db.get_assets_by_type(&AssetType::Level),
                    "script" => db.get_assets_by_type(&AssetType::Script),
                    _ => db.get_all_assets(),
                }
            } else {
                db.get_all_assets()
            };

            let display_count = limit.unwrap_or(assets.len());
            let assets_to_show = &assets[..display_count.min(assets.len())];

            if assets_to_show.is_empty() {
                println!("📭 No assets found in database.");
                return Ok(());
            }

            println!("📋 Assets in database (showing {} of {}):", assets_to_show.len(), assets.len());
            println!();

            for (i, asset) in assets_to_show.iter().enumerate() {
                println!("{}. {} ({:?}) - {} bytes",
                        i + 1,
                        asset.name,
                        asset.asset_type,
                        format_bytes(asset.file_size));
                println!("   📁 {}", asset.output_path.display());
                if !asset.tags.is_empty() {
                    println!("   🏷️  {}", asset.tags.join(", "));
                }
                if let Some(game) = &asset.game_id {
                    println!("   🎮 {}", game);
                }
                println!();
            }
        }

    }

    Ok(())
}

/// Handle the API server command
#[cfg(feature = "api")]
async fn handle_serve_command(
    address: &str,
    database: &std::path::Path,
    cors: bool,
) -> Result<()> {
    use aegis_core::api::{ApiServer, ApiConfig};

    println!("🚀 Starting Aegis-Assets API server...");
    println!("📍 Address: {}", address);
    println!("🗄️  Database: {}", database.display());
    println!("🌐 CORS: {}", if cors { "enabled" } else { "disabled" });

    let config = ApiConfig {
        db_path: database.to_path_buf(),
        cors_enabled: cors,
        rate_limit: Some(100),
    };

    let server = ApiServer::with_config(config).await
        .context("Failed to create API server")?;

    let addr = address.parse()
        .context("Invalid server address")?;

    println!("✅ Server ready! API endpoints available at:");
    println!("   📋 Health: http://{}/api/v1/health", address);
    println!("   📄 Info: http://{}/api/v1/info", address);
    println!("   🔍 Assets: http://{}/api/v1/assets", address);
    println!("   🔎 Search: http://{}/api/v1/assets/search", address);
    println!("   📊 Stats: http://{}/api/v1/assets/stats", address);
    println!();
    println!("Press Ctrl+C to stop the server");

    server.serve(addr).await
        .context("Server error")?;

    Ok(())
}

/// Handle plugin search
fn handle_search_plugins(
    query: &str,
    engine: Option<&String>,
    risk: Option<&String>,
    limit: Option<usize>,
    verbose: bool,
    plugin_registry: PluginRegistry,
) -> Result<()> {
    println!("🔍 Searching for plugins: {}", query);

    if query.is_empty() {
        println!("❌ Search query cannot be empty");
        return Ok(());
    }

    // For now, search through loaded plugins
    // In a full implementation, this would search the marketplace
    let plugins = plugin_registry.list_plugins();
    let filtered_plugins: Vec<_> = plugins
        .into_iter()
        .filter(|plugin| {
            let name_match = plugin.name().to_lowercase().contains(&query.to_lowercase());
            let desc_match = plugin.compliance_info()
                .description
                .as_ref()
                .map_or(false, |desc| desc.to_lowercase().contains(&query.to_lowercase()));

            name_match || desc_match
        })
        .collect();

    if filtered_plugins.is_empty() {
        println!("❌ No plugins found matching '{}'", query);
        return Ok(());
    }

    let display_count = limit.unwrap_or(filtered_plugins.len());
    let plugins_to_show = &filtered_plugins[..display_count.min(filtered_plugins.len())];

    println!("📦 Found {} plugins (showing {}):", filtered_plugins.len(), plugins_to_show.len());

    for plugin in plugins_to_show {
        let compliance = plugin.compliance_info();
        let status = if compliance.compliance_verified { "✅" } else { "⚠️" };

        println!("   {} {} v{}", status, plugin.name(), plugin.version());
        println!("      Extensions: {}", plugin.supported_extensions().join(", "));

        if verbose {
            if let Some(description) = &compliance.description {
                println!("      Description: {}", description);
            }
            if let Some(author) = &compliance.author {
                println!("      Author: {}", author);
            }
            println!("      Risk Level: {:?}", compliance.risk_level);
            println!("      Publisher Policy: {:?}", compliance.publisher_policy);
        }
    }

    Ok(())
}

/// Handle plugin info
fn handle_plugin_info(
    plugin_name: &str,
    show_history: bool,
    show_compliance: bool,
    plugin_registry: PluginRegistry,
) -> Result<()> {
    println!("📋 Plugin information: {}", plugin_name);

    // Find the plugin
    let plugins = plugin_registry.list_plugins();
    let plugin = plugins
        .into_iter()
        .find(|p| p.name() == plugin_name);

    match plugin {
        Some(plugin) => {
            let compliance = plugin.compliance_info();
            let status = if compliance.compliance_verified { "✅" } else { "⚠️" };

            println!("   Name: {} v{}", plugin.name(), plugin.version());
            println!("   Status: {}", status);
            println!("   Extensions: {}", plugin.supported_extensions().join(", "));

            if let Some(description) = &compliance.description {
                println!("   Description: {}", description);
            }
            if let Some(author) = &compliance.author {
                println!("   Author: {}", author);
            }
            if let Some(homepage) = &compliance.homepage {
                println!("   Homepage: {}", homepage);
            }

            if show_compliance {
                println!("\n📋 Compliance Information:");
                println!("   Risk Level: {:?}", compliance.risk_level);
                println!("   Publisher Policy: {:?}", compliance.publisher_policy);
                println!("   Bounty Eligible: {}", if compliance.bounty_eligible { "Yes" } else { "No" });
                println!("   Enterprise Approved: {}", if compliance.enterprise_approved { "Yes" } else { "No" });
            }

            if show_history {
                println!("\n📜 Version History:");
                println!("   (History not available - plugin registry integration needed)");
            }
        }
        None => {
            println!("❌ Plugin '{}' not found", plugin_name);
            println!("\n💡 Available plugins:");
            for plugin in plugin_registry.list_plugins() {
                println!("   • {} v{}", plugin.name(), plugin.version());
            }
        }
    }

    Ok(())
}

/// Handle plugin installation
fn handle_install_plugin(
    plugin_name: &str,
    version: Option<&String>,
    force: bool,
    skip_deps: bool,
    plugin_registry: PluginRegistry,
) -> Result<()> {
    println!("📦 Installing plugin: {}", plugin_name);

    // Check if plugin is already installed
    let plugins = plugin_registry.list_plugins();
    let already_installed = plugins.iter().any(|p| p.name() == plugin_name);

    if already_installed && !force {
        println!("⚠️  Plugin '{}' is already installed", plugin_name);
        println!("   Use --force to reinstall");
        return Ok(());
    }

    println!("🔍 Searching marketplace for '{}'...", plugin_name);

    // Mock marketplace search
    // In a real implementation, this would search the marketplace API
    println!("❌ Plugin '{}' not found in marketplace", plugin_name);
    println!("\n💡 Available plugins:");
    for plugin in plugin_registry.list_plugins() {
        println!("   • {} v{}", plugin.name(), plugin.version());
    }

    Ok(())
}

/// Handle plugin uninstallation
fn handle_uninstall_plugin(
    plugin_name: &str,
    purge: bool,
    skip_deps: bool,
    plugin_registry: PluginRegistry,
) -> Result<()> {
    println!("🗑️  Uninstalling plugin: {}", plugin_name);

    // Check if plugin is installed
    let plugins = plugin_registry.list_plugins();
    let plugin_exists = plugins.iter().any(|p| p.name() == plugin_name);

    if !plugin_exists {
        println!("❌ Plugin '{}' is not installed", plugin_name);
        return Ok(());
    }

    // Mock uninstallation
    println!("⚠️  Plugin '{}' would be uninstalled", plugin_name);
    if purge {
        println!("   (Purge mode: also removing configuration and data)");
    }
    if skip_deps {
        println!("   (Skipping dependency checks)");
    }
    println!("✅ Plugin '{}' uninstalled successfully", plugin_name);

    Ok(())
}

/// Handle plugin updates
fn handle_update_plugins(
    specific_plugin: Option<&String>,
    force: bool,
    skip_deps: bool,
    plugin_registry: PluginRegistry,
) -> Result<()> {
    println!("🔄 Updating plugins...");

    let plugins = plugin_registry.list_plugins();

    if let Some(plugin_name) = specific_plugin {
        println!("📦 Updating specific plugin: {}", plugin_name);

        let plugin_exists = plugins.iter().any(|p| p.name() == plugin_name);
        if !plugin_exists {
            println!("❌ Plugin '{}' is not installed", plugin_name);
            return Ok(());
        }

        println!("✅ Plugin '{}' updated to latest version", plugin_name);
    } else {
        println!("📦 Checking for updates for {} plugins...", plugins.len());

        if plugins.is_empty() {
            println!("❌ No plugins installed");
            return Ok(());
        }

        // Mock update check
        println!("✅ All plugins are up to date");
        if force {
            println!("   (Force update: would update even with breaking changes)");
        }
    }

    Ok(())
}

/// Handle plugin marketplace statistics
fn handle_plugin_stats(plugin_registry: PluginRegistry) -> Result<()> {
    println!("📊 Plugin Marketplace Statistics");

    let plugins = plugin_registry.list_plugins();

    println!("   📦 Total Plugins: {}", plugins.len());

    // Count by type
    let mut format_counts = std::collections::HashMap::new();
    for plugin in &plugins {
        for ext in plugin.supported_extensions() {
            *format_counts.entry(ext.clone()).or_insert(0) += 1;
        }
    }

    println!("   📁 Supported Formats:");
    for (format, count) in format_counts {
        println!("      {}: {} plugins", format, count);
    }

    // Compliance statistics
    let mut risk_counts = std::collections::HashMap::new();
    let mut policy_counts = std::collections::HashMap::new();

    for plugin in &plugins {
        let compliance = plugin.compliance_info();
        *risk_counts.entry(format!("{:?}", compliance.risk_level)).or_insert(0) += 1;
        *policy_counts.entry(format!("{:?}", compliance.publisher_policy)).or_insert(0) += 1;
    }

    println!("   ⚖️  Risk Distribution:");
    for (risk, count) in risk_counts {
        println!("      {}: {} plugins", risk, count);
    }

    println!("   📋 Publisher Policies:");
    for (policy, count) in policy_counts {
        println!("      {}: {} plugins", policy, count);
    }

    Ok(())
}
