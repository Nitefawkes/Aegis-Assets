use anyhow::{Result, Context};
use clap::{Args, Subcommand};
use std::path::PathBuf;
use aegis_core::{AegisCore, ComplianceLevel};
use crate::ui::{success, warning, error, info};
use colored::*;
use tabled::{Table, Tabled};

/// Check compliance status and manage profiles
#[derive(Args)]
pub struct ComplianceCommand {
    #[command(subcommand)]
    pub action: ComplianceAction,
}

#[derive(Subcommand)]
pub enum ComplianceAction {
    /// Check compliance status for a specific game/publisher
    Check {
        /// Game identifier or file path
        target: String,
        
        /// Show detailed compliance analysis
        #[arg(long)]
        detailed: bool,
    },
    
    /// List all loaded compliance profiles
    List {
        /// Filter by enforcement level
        #[arg(long)]
        level: Option<ComplianceLevelFilter>,
        
        /// Show detailed profile information
        #[arg(long)]
        detailed: bool,
    },
    
    /// Show compliance information for a specific profile
    Show {
        /// Publisher or profile name
        profile: String,
    },
    
    /// Generate a compliance report
    Report {
        /// Directory to scan for potential compliance issues
        #[arg(long)]
        scan_dir: Option<PathBuf>,
        
        /// Output format (json, yaml, markdown)
        #[arg(long, default_value = "markdown")]
        format: ReportFormat,
        
        /// Output file (stdout if not specified)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    
    /// Update compliance profiles from remote source
    Update {
        /// URL or path to updated profiles
        #[arg(long)]
        source: Option<String>,
        
        /// Backup existing profiles before update
        #[arg(long)]
        backup: bool,
    },
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum ComplianceLevelFilter {
    Permissive,
    Neutral,
    HighRisk,
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum ReportFormat {
    Json,
    Yaml,
    Markdown,
}

#[derive(Tabled)]
struct ComplianceProfileSummary {
    #[tabled(rename = "Publisher")]
    publisher: String,
    
    #[tabled(rename = "Level")]
    level: String,
    
    #[tabled(rename = "Official Support")]
    official_support: String,
    
    #[tabled(rename = "Formats")]
    format_count: usize,
    
    #[tabled(rename = "Notes")]
    notes: String,
}

impl ComplianceCommand {
    pub fn execute(&self, core: &AegisCore) -> Result<()> {
        match &self.action {
            ComplianceAction::Check { target, detailed } => {
                self.check_compliance(core, target, *detailed)
            }
            ComplianceAction::List { level, detailed } => {
                self.list_profiles(core, level.as_ref(), *detailed)
            }
            ComplianceAction::Show { profile } => {
                self.show_profile(core, profile)
            }
            ComplianceAction::Report { scan_dir, format, output } => {
                self.generate_report(core, scan_dir.as_ref(), format, output.as_ref())
            }
            ComplianceAction::Update { source, backup } => {
                self.update_profiles(core, source.as_ref(), *backup)
            }
        }
    }
    
    fn check_compliance(&self, core: &AegisCore, target: &str, detailed: bool) -> Result<()> {
        info(&format!("Checking compliance for: {}", target));
        
        // Determine if target is a file path or game identifier
        let game_id = if std::path::Path::new(target).exists() {
            // Try to detect game from file
            self.detect_game_from_file(target)?
        } else {
            target.to_string()
        };
        
        println!("{}", "Compliance Check Results:".bright_blue().bold());
        println!("  Target: {}", game_id);
        
        // This would use the actual compliance checker in a real implementation
        // For now, provide mock compliance check based on known patterns
        let (level, messages) = self.mock_compliance_check(&game_id);
        
        match level {
            ComplianceLevel::Permissive => {
                success("✅ PERMISSIVE - Extraction generally allowed");
            }
            ComplianceLevel::Neutral => {
                warning("⚠  NEUTRAL - Proceed with caution");
            }
            ComplianceLevel::HighRisk => {
                error("🚫 HIGH RISK - Strong IP enforcement");
            }
        }
        
        if !messages.is_empty() {
            println!("\n{}", "Details:".bright_yellow().bold());
            for message in messages {
                println!("  • {}", message);
            }
        }
        
        if detailed {
            println!("\n{}", "Recommendations:".bright_yellow().bold());
            match level {
                ComplianceLevel::Permissive => {
                    println!("  • Extraction is generally supported");
                    println!("  • Check for official modding tools");
                    println!("  • Respect game-specific EULAs");
                }
                ComplianceLevel::Neutral => {
                    println!("  • Ensure you own the original game");
                    println!("  • Use extracted assets only for personal projects");
                    println!("  • Do not redistribute extracted assets");
                    println!("  • Monitor for changes in publisher policy");
                }
                ComplianceLevel::HighRisk => {
                    println!("  • Consider using official tools if available");
                    println!("  • Avoid commercial or public use");
                    println!("  • Enable strict compliance mode");
                    println!("  • Consult legal counsel for institutional use");
                }
            }
        }
        
        Ok(())
    }
    
    fn list_profiles(&self, core: &AegisCore, level_filter: Option<&ComplianceLevelFilter>, detailed: bool) -> Result<()> {
        info("Loading compliance profiles...");
        
        let system_info = core.system_info();
        
        if system_info.compliance_profiles == 0 {
            warning("No compliance profiles loaded");
            println!("Use --compliance-profiles to specify a profiles directory");
            return Ok(());
        }
        
        println!("{}", "Loaded Compliance Profiles:".bright_blue().bold());
        
        // Mock profile data for demonstration
        let mock_profiles = self.get_mock_profiles();
        
        let filtered_profiles: Vec<_> = mock_profiles
            .into_iter()
            .filter(|profile| {
                if let Some(filter) = level_filter {
                    match (filter, profile.level.as_str()) {
                        (ComplianceLevelFilter::Permissive, "Permissive") => true,
                        (ComplianceLevelFilter::Neutral, "Neutral") => true,
                        (ComplianceLevelFilter::HighRisk, "High Risk") => true,
                        _ => false,
                    }
                } else {
                    true
                }
            })
            .collect();
        
        if filtered_profiles.is_empty() {
            warning("No profiles match the specified filter");
            return Ok(());
        }
        
        if detailed {
            for profile in filtered_profiles {
                self.print_detailed_profile(&profile);
            }
        } else {
            let table = Table::new(filtered_profiles);
            println!("{}", table);
        }
        
        Ok(())
    }
    
    fn show_profile(&self, core: &AegisCore, profile_name: &str) -> Result<()> {
        info(&format!("Loading profile: {}", profile_name));
        
        // Mock profile lookup
        let mock_profiles = self.get_mock_profiles();
        let profile = mock_profiles
            .iter()
            .find(|p| p.publisher.to_lowercase().contains(&profile_name.to_lowercase()))
            .ok_or_else(|| anyhow::anyhow!("Profile not found: {}", profile_name))?;
        
        self.print_detailed_profile(profile);
        
        Ok(())
    }
    
    fn generate_report(&self, core: &AegisCore, scan_dir: Option<&PathBuf>, format: &ReportFormat, output: Option<&PathBuf>) -> Result<()> {
        info("Generating compliance report...");
        
        let report_content = self.create_compliance_report(core, scan_dir)?;
        
        let output_text = match format {
            ReportFormat::Json => serde_json::to_string_pretty(&report_content)?,
            ReportFormat::Yaml => serde_yaml::to_string(&report_content)?,
            ReportFormat::Markdown => self.format_report_as_markdown(&report_content),
        };
        
        if let Some(output_path) = output {
            std::fs::write(output_path, &output_text)
                .context("Failed to write report file")?;
            success(&format!("Report written to: {}", output_path.display()));
        } else {
            println!("{}", output_text);
        }
        
        Ok(())
    }
    
    fn update_profiles(&self, core: &AegisCore, source: Option<&String>, backup: bool) -> Result<()> {
        info("Updating compliance profiles...");
        
        if backup {
            info("Creating backup of existing profiles...");
            // Would implement backup functionality here
        }
        
        match source {
            Some(url) => {
                info(&format!("Downloading profiles from: {}", url));
                // Would implement profile download/update here
                warning("Profile updates are not implemented yet");
            }
            None => {
                info("Using default profile repository...");
                warning("Default profile updates are not implemented yet");
            }
        }
        
        println!("{}", "Update Status:".bright_blue().bold());
        println!("  • Profile updates are planned for a future release");
        println!("  • Currently using local profiles only");
        println!("  • Check the repository for the latest profiles");
        
        Ok(())
    }
    
    fn detect_game_from_file(&self, file_path: &str) -> Result<String> {
        let path = std::path::Path::new(file_path);
        
        // Try to detect game from filename or path
        let file_name = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_lowercase();
        
        if file_name.contains("unity") {
            Ok("unity_generic".to_string())
        } else if file_name.contains("unreal") {
            Ok("unreal_generic".to_string())
        } else if file_name.contains("skyrim") || file_name.contains("fallout") {
            Ok("bethesda_game".to_string())
        } else {
            Ok("unknown_game".to_string())
        }
    }
    
    fn mock_compliance_check(&self, game_id: &str) -> (ComplianceLevel, Vec<String>) {
        let lower_id = game_id.to_lowercase();
        
        if lower_id.contains("bethesda") || lower_id.contains("skyrim") || lower_id.contains("fallout") {
            (
                ComplianceLevel::Permissive,
                vec![
                    "Bethesda actively supports modding".to_string(),
                    "Official Creation Kit available".to_string(),
                    "Large modding community".to_string(),
                ]
            )
        } else if lower_id.contains("nintendo") || lower_id.contains("switch") {
            (
                ComplianceLevel::HighRisk,
                vec![
                    "Nintendo aggressively enforces IP rights".to_string(),
                    "History of DMCA takedowns".to_string(),
                    "No official modding support".to_string(),
                ]
            )
        } else if lower_id.contains("unity") {
            (
                ComplianceLevel::Neutral,
                vec![
                    "Unity engine used by many developers".to_string(),
                    "Publisher policies vary widely".to_string(),
                    "Check specific game publisher".to_string(),
                ]
            )
        } else {
            (
                ComplianceLevel::Neutral,
                vec![
                    "Unknown publisher policy".to_string(),
                    "Proceed with standard cautions".to_string(),
                ]
            )
        }
    }
    
    fn get_mock_profiles(&self) -> Vec<ComplianceProfileSummary> {
        vec![
            ComplianceProfileSummary {
                publisher: "Bethesda Game Studios".to_string(),
                level: "Permissive".bright_green().to_string(),
                official_support: "Yes".to_string(),
                format_count: 6,
                notes: "Supports modding".to_string(),
            },
            ComplianceProfileSummary {
                publisher: "Nintendo".to_string(),
                level: "High Risk".red().to_string(),
                official_support: "No".to_string(),
                format_count: 6,
                notes: "Aggressive IP enforcement".to_string(),
            },
            ComplianceProfileSummary {
                publisher: "Valve Corporation".to_string(),
                level: "Permissive".bright_green().to_string(),
                official_support: "Yes".to_string(),
                format_count: 9,
                notes: "Strong modding support".to_string(),
            },
            ComplianceProfileSummary {
                publisher: "Unity Technologies".to_string(),
                level: "Neutral".yellow().to_string(),
                official_support: "Varies".to_string(),
                format_count: 5,
                notes: "Engine-level profile".to_string(),
            },
        ]
    }
    
    fn print_detailed_profile(&self, profile: &ComplianceProfileSummary) {
        println!("\n{}", format!("Profile: {}", profile.publisher).bright_blue().bold());
        println!("  Enforcement Level: {}", profile.level);
        println!("  Official Support: {}", profile.official_support);
        println!("  Supported Formats: {}", profile.format_count);
        println!("  Notes: {}", profile.notes);
        
        // Additional mock details
        println!("  Policy URL: {}", "https://example.com/modding-policy".dimmed());
        println!("  Last Updated: {}", "2024-01-15".dimmed());
        println!("  Bounty Eligible: {}", if profile.level.contains("Permissive") { "Yes" } else { "No" });
    }
    
    fn create_compliance_report(&self, core: &AegisCore, scan_dir: Option<&PathBuf>) -> Result<serde_json::Value> {
        let system_info = core.system_info();
        
        let mut report = serde_json::json!({
            "report_type": "compliance_analysis",
            "generated_at": chrono::Utc::now().to_rfc3339(),
            "aegis_version": system_info.version,
            "profiles_loaded": system_info.compliance_profiles,
            "summary": {
                "permissive_publishers": 2,
                "neutral_publishers": 1,
                "high_risk_publishers": 1,
                "total_formats_supported": 26
            }
        });
        
        if let Some(dir) = scan_dir {
            info(&format!("Scanning directory: {}", dir.display()));
            let scan_results = self.scan_directory_for_compliance(dir)?;
            report["scan_results"] = scan_results;
        }
        
        Ok(report)
    }
    
    fn scan_directory_for_compliance(&self, dir: &PathBuf) -> Result<serde_json::Value> {
        let mut findings = Vec::new();
        
        // Mock directory scan
        findings.push(serde_json::json!({
            "file": "game.unity3d",
            "detected_engine": "Unity",
            "compliance_level": "Neutral",
            "recommendations": ["Verify publisher policy", "Use for personal projects only"]
        }));
        
        Ok(serde_json::json!({
            "files_scanned": 1,
            "findings": findings,
            "risk_summary": {
                "low_risk": 0,
                "medium_risk": 1,
                "high_risk": 0
            }
        }))
    }
    
    fn format_report_as_markdown(&self, report: &serde_json::Value) -> String {
        format!(r#"# Aegis-Assets Compliance Report

Generated: {}
Aegis Version: {}

## Summary

- Compliance Profiles Loaded: {}
- Permissive Publishers: {}
- Neutral Publishers: {}
- High-Risk Publishers: {}

## Recommendations

1. **Keep profiles updated** - Check for profile updates regularly
2. **Enable strict mode** - For institutional use, enable strict compliance mode
3. **Maintain audit logs** - Track all extraction activities
4. **Review publisher policies** - Monitor changes in publisher modding policies

## Profile Status

| Publisher | Level | Support | Formats |
|-----------|-------|---------|---------|
| Bethesda Game Studios | ✅ Permissive | Official | 6 |
| Valve Corporation | ✅ Permissive | Official | 9 |
| Unity Technologies | ⚠️ Neutral | Varies | 5 |
| Nintendo | 🚫 High Risk | None | 6 |

---

*This report was generated automatically by Aegis-Assets.*
"#,
            report.get("generated_at").and_then(|v| v.as_str()).unwrap_or("Unknown"),
            report.get("aegis_version").and_then(|v| v.as_str()).unwrap_or("Unknown"),
            report.get("profiles_loaded").and_then(|v| v.as_u64()).unwrap_or(0),
            report.get("summary").and_then(|s| s.get("permissive_publishers")).and_then(|v| v.as_u64()).unwrap_or(0),
            report.get("summary").and_then(|s| s.get("neutral_publishers")).and_then(|v| v.as_u64()).unwrap_or(0),
            report.get("summary").and_then(|s| s.get("high_risk_publishers")).and_then(|v| v.as_u64()).unwrap_or(0),
        )
    }
}
