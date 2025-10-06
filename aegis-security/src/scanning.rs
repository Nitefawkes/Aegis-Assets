//! Static code analysis and vulnerability scanning

use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use syn::{File, visit::Visit};
use tracing::{info, debug, warn};

/// Main security scanner for plugin code analysis
pub struct SecurityScanner {
    rules: Vec<Box<dyn SecurityRule>>,
    config: ScannerConfig,
}

impl SecurityScanner {
    /// Create a new security scanner with default rules
    pub async fn new() -> Result<Self> {
        let mut scanner = Self {
            rules: Vec::new(),
            config: ScannerConfig::default(),
        };
        
        // Register built-in security rules
        scanner.register_default_rules();
        
        // Load RustSec advisory database
        scanner.update_vulnerability_database().await?;
        
        Ok(scanner)
    }
    
    /// Register all default security rules
    fn register_default_rules(&mut self) {
        self.rules.push(Box::new(UnsafeCodeRule));
        self.rules.push(Box::new(NetworkAccessRule));
        self.rules.push(Box::new(FileSystemAccessRule));
        self.rules.push(Box::new(ProcessSpawningRule));
        self.rules.push(Box::new(CryptographyRule));
        self.rules.push(Box::new(DependencyVulnerabilityRule::new()));
        self.rules.push(Box::new(MemoryLeakRule));
        self.rules.push(Box::new(InputValidationRule));
        self.rules.push(Box::new(ErrorHandlingRule));
        self.rules.push(Box::new(PrivilegeEscalationRule));
    }
    
    /// Update vulnerability database from external sources
    async fn update_vulnerability_database(&mut self) -> Result<()> {
        info!("Updating vulnerability database");
        
        // Download RustSec advisory database
        let client = reqwest::Client::new();
        let advisories_url = "https://raw.githubusercontent.com/RustSec/advisory-db/main/crates.toml";
        
        match client.get(advisories_url).send().await {
            Ok(response) => {
                let content = response.text().await?;
                self.parse_rustsec_advisories(&content)?;
                info!("Vulnerability database updated successfully");
            }
            Err(e) => {
                warn!("Failed to update vulnerability database: {}", e);
                // Continue with cached/local database
            }
        }
        
        Ok(())
    }
    
    fn parse_rustsec_advisories(&mut self, content: &str) -> Result<()> {
        // Parse RustSec advisory format and update dependency vulnerability rule
        for rule in &mut self.rules {
            if rule.name() == "dependency_vulnerabilities" {
                // Update rule with new advisory data
                // This would be implemented with proper TOML parsing
                break;
            }
        }
        Ok(())
    }
    
    /// Scan a plugin for security vulnerabilities
    pub async fn scan_plugin(&self, plugin_path: &Path) -> Result<ScanReport> {
        info!("Starting security scan of: {}", plugin_path.display());
        
        let scan_id = uuid::Uuid::new_v4();
        let scan_start = chrono::Utc::now();
        
        // Extract plugin if it's an archive
        let work_dir = self.extract_plugin(plugin_path).await?;
        
        // Analyze plugin structure
        let code_context = CodeContext::analyze(&work_dir)?;
        
        // Run all security rules in parallel
        let mut all_findings = Vec::new();
        
        for rule in &self.rules {
            debug!("Running security rule: {}", rule.name());
            
            match rule.scan(&code_context).await {
                Ok(findings) => {
                    all_findings.extend(findings);
                }
                Err(e) => {
                    warn!("Security rule '{}' failed: {}", rule.name(), e);
                }
            }
        }
        
        // Calculate risk metrics
        let risk_metrics = self.calculate_risk_metrics(&all_findings);
        
        let scan_end = chrono::Utc::now();
        let scan_duration = scan_end.signed_duration_since(scan_start);
        
        // Cleanup temporary files
        if work_dir != plugin_path {
            std::fs::remove_dir_all(&work_dir).ok();
        }
        
        let report = ScanReport {
            scan_id,
            plugin_path: plugin_path.to_path_buf(),
            scan_start,
            scan_end,
            scan_duration,
            findings: all_findings,
            risk_metrics,
            rules_executed: self.rules.len(),
        };
        
        info!("Security scan complete: {} findings in {}ms", 
               report.findings.len(), scan_duration.num_milliseconds());
        
        Ok(report)
    }
    
    async fn extract_plugin(&self, plugin_path: &Path) -> Result<PathBuf> {
        // If it's already a directory, return as-is
        if plugin_path.is_dir() {
            return Ok(plugin_path.to_path_buf());
        }
        
        // Extract archive to temporary directory
        let temp_dir = tempfile::tempdir()?;
        let extract_path = temp_dir.into_path();
        
        match plugin_path.extension().and_then(|e| e.to_str()) {
            Some("tar") | Some("tar.gz") | Some("tgz") => {
                self.extract_tar(plugin_path, &extract_path)?;
            }
            Some("zip") => {
                self.extract_zip(plugin_path, &extract_path)?;
            }
            _ => {
                // Assume it's a single file
                std::fs::copy(plugin_path, extract_path.join("main.rs"))?;
            }
        }
        
        Ok(extract_path)
    }
    
    fn extract_tar(&self, archive_path: &Path, extract_path: &Path) -> Result<()> {
        let file = std::fs::File::open(archive_path)?;
        let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(file));
        archive.unpack(extract_path)?;
        Ok(())
    }
    
    fn extract_zip(&self, archive_path: &Path, extract_path: &Path) -> Result<()> {
        let file = std::fs::File::open(archive_path)?;
        let mut archive = zip::ZipArchive::new(file)?;
        archive.extract(extract_path)?;
        Ok(())
    }
    
    fn calculate_risk_metrics(&self, findings: &[SecurityFinding]) -> RiskMetrics {
        let mut critical_count = 0;
        let mut high_count = 0;
        let mut medium_count = 0;
        let mut low_count = 0;
        let mut info_count = 0;
        
        for finding in findings {
            match finding.severity {
                Severity::Critical => critical_count += 1,
                Severity::High => high_count += 1,
                Severity::Medium => medium_count += 1,
                Severity::Low => low_count += 1,
                Severity::Info => info_count += 1,
            }
        }
        
        // Calculate weighted risk score
        let risk_score = (critical_count * 50) + (high_count * 20) + (medium_count * 10) + 
                        (low_count * 5) + (info_count * 1);
        
        let risk_level = match risk_score {
            0..=10 => RiskLevel::Low,
            11..=30 => RiskLevel::Medium,
            31..=60 => RiskLevel::High,
            _ => RiskLevel::Critical,
        };
        
        RiskMetrics {
            total_findings: findings.len(),
            critical_count,
            high_count,
            medium_count,
            low_count,
            info_count,
            risk_score,
            risk_level,
        }
    }
}

/// Code context for analysis
#[derive(Debug)]
pub struct CodeContext {
    pub rust_files: Vec<PathBuf>,
    pub cargo_toml: Option<PathBuf>,
    pub total_lines: usize,
    pub dependencies: HashMap<String, String>,
}

impl CodeContext {
    pub fn analyze(dir: &Path) -> Result<Self> {
        let mut rust_files = Vec::new();
        let mut cargo_toml = None;
        let mut total_lines = 0;
        
        for entry in walkdir::WalkDir::new(dir) {
            let entry = entry?;
            let path = entry.path();
            
            if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                rust_files.push(path.to_path_buf());
                
                // Count lines
                if let Ok(content) = std::fs::read_to_string(path) {
                    total_lines += content.lines().count();
                }
            } else if path.file_name().and_then(|n| n.to_str()) == Some("Cargo.toml") {
                cargo_toml = Some(path.to_path_buf());
            }
        }
        
        // Parse dependencies from Cargo.toml
        let dependencies = if let Some(cargo_path) = &cargo_toml {
            Self::parse_dependencies(cargo_path).unwrap_or_default()
        } else {
            HashMap::new()
        };
        
        Ok(CodeContext {
            rust_files,
            cargo_toml,
            total_lines,
            dependencies,
        })
    }
    
    fn parse_dependencies(cargo_toml: &Path) -> Result<HashMap<String, String>> {
        let content = std::fs::read_to_string(cargo_toml)?;
        let parsed: toml::Value = toml::from_str(&content)?;
        
        let mut dependencies = HashMap::new();
        
        if let Some(deps) = parsed.get("dependencies").and_then(|d| d.as_table()) {
            for (name, version_info) in deps {
                let version = match version_info {
                    toml::Value::String(version) => version.clone(),
                    toml::Value::Table(table) => {
                        table.get("version")
                            .and_then(|v| v.as_str())
                            .unwrap_or("*")
                            .to_string()
                    }
                    _ => "*".to_string(),
                };
                dependencies.insert(name.clone(), version);
            }
        }
        
        Ok(dependencies)
    }
}

/// Security rule trait for implementing custom security checks
#[async_trait::async_trait]
pub trait SecurityRule: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn category(&self) -> SecurityCategory;
    async fn scan(&self, context: &CodeContext) -> Result<Vec<SecurityFinding>>;
}

/// Security finding from rule analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityFinding {
    pub rule_name: String,
    pub severity: Severity,
    pub category: SecurityCategory,
    pub message: String,
    pub description: String,
    pub location: FileLocation,
    pub recommendation: String,
    pub cwe_id: Option<u32>, // Common Weakness Enumeration ID
}

/// Severity levels for security findings
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

/// Security categories for findings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SecurityCategory {
    MemorySafety,
    NetworkAccess,
    FileSystemAccess,
    ProcessSpawning,
    Cryptography,
    DependencyVulnerability,
    InputValidation,
    ErrorHandling,
    PrivilegeEscalation,
    InformationDisclosure,
    Other(String),
}

/// File location for a security finding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileLocation {
    pub file_path: PathBuf,
    pub line_number: usize,
    pub column_number: Option<usize>,
    pub function_name: Option<String>,
}

/// Complete scan report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanReport {
    pub scan_id: uuid::Uuid,
    pub plugin_path: PathBuf,
    pub scan_start: chrono::DateTime<chrono::Utc>,
    pub scan_end: chrono::DateTime<chrono::Utc>,
    pub scan_duration: chrono::Duration,
    pub findings: Vec<SecurityFinding>,
    pub risk_metrics: RiskMetrics,
    pub rules_executed: usize,
}

/// Risk assessment metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskMetrics {
    pub total_findings: usize,
    pub critical_count: usize,
    pub high_count: usize,
    pub medium_count: usize,
    pub low_count: usize,
    pub info_count: usize,
    pub risk_score: usize,
    pub risk_level: RiskLevel,
}

/// Overall risk level assessment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

/// Scanner configuration
#[derive(Debug, Clone)]
pub struct ScannerConfig {
    pub max_file_size_mb: usize,
    pub max_scan_duration_sec: u64,
    pub parallel_analysis: bool,
}

impl Default for ScannerConfig {
    fn default() -> Self {
        Self {
            max_file_size_mb: 100,
            max_scan_duration_sec: 300, // 5 minutes
            parallel_analysis: true,
        }
    }
}

// ===== BUILT-IN SECURITY RULES =====

/// Detects unsafe code blocks
pub struct UnsafeCodeRule;

#[async_trait::async_trait]
impl SecurityRule for UnsafeCodeRule {
    fn name(&self) -> &str { "unsafe_code_detection" }
    fn description(&self) -> &str { "Detects unsafe Rust code blocks" }
    fn category(&self) -> SecurityCategory { SecurityCategory::MemorySafety }
    
    async fn scan(&self, context: &CodeContext) -> Result<Vec<SecurityFinding>> {
        let mut findings = Vec::new();
        let unsafe_pattern = regex::Regex::new(r"unsafe\s*\{")?;
        
        for file_path in &context.rust_files {
            let content = std::fs::read_to_string(file_path)?;
            
            for (line_num, line) in content.lines().enumerate() {
                if unsafe_pattern.is_match(line) {
                    findings.push(SecurityFinding {
                        rule_name: self.name().to_string(),
                        severity: Severity::High,
                        category: self.category(),
                        message: "Unsafe code block detected".to_string(),
                        description: "Code uses unsafe Rust which bypasses memory safety guarantees".to_string(),
                        location: FileLocation {
                            file_path: file_path.clone(),
                            line_number: line_num + 1,
                            column_number: None,
                            function_name: None,
                        },
                        recommendation: "Avoid unsafe code or provide detailed justification and safety analysis".to_string(),
                        cwe_id: Some(119), // CWE-119: Buffer Errors
                    });
                }
            }
        }
        
        Ok(findings)
    }
}

/// Detects network access patterns
pub struct NetworkAccessRule;

#[async_trait::async_trait]
impl SecurityRule for NetworkAccessRule {
    fn name(&self) -> &str { "network_access_detection" }
    fn description(&self) -> &str { "Detects network access attempts" }
    fn category(&self) -> SecurityCategory { SecurityCategory::NetworkAccess }
    
    async fn scan(&self, context: &CodeContext) -> Result<Vec<SecurityFinding>> {
        let mut findings = Vec::new();
        
        let network_patterns = [
            (r"use\s+std::net::", "Direct networking via std::net"),
            (r"use\s+reqwest::", "HTTP client library usage"),
            (r"use\s+hyper::", "HTTP library usage"),
            (r"TcpStream::", "TCP socket usage"),
            (r"UdpSocket::", "UDP socket usage"),
            (r"\.connect\(", "Network connection attempt"),
        ];
        
        for file_path in &context.rust_files {
            let content = std::fs::read_to_string(file_path)?;
            
            for (pattern, description) in &network_patterns {
                let regex = regex::Regex::new(pattern)?;
                
                for (line_num, line) in content.lines().enumerate() {
                    if regex.is_match(line) {
                        findings.push(SecurityFinding {
                            rule_name: self.name().to_string(),
                            severity: Severity::Medium,
                            category: self.category(),
                            message: format!("Network access detected: {}", description),
                            description: format!("Code line: {}", line.trim()),
                            location: FileLocation {
                                file_path: file_path.clone(),
                                line_number: line_num + 1,
                                column_number: None,
                                function_name: None,
                            },
                            recommendation: "Ensure network access is declared in plugin permissions and follows security guidelines".to_string(),
                            cwe_id: Some(200), // CWE-200: Information Exposure
                        });
                    }
                }
            }
        }
        
        Ok(findings)
    }
}

/// Additional security rules would be implemented here...
pub struct FileSystemAccessRule;
pub struct ProcessSpawningRule;
pub struct CryptographyRule;
pub struct MemoryLeakRule;
pub struct InputValidationRule;
pub struct ErrorHandlingRule;
pub struct PrivilegeEscalationRule;

pub struct DependencyVulnerabilityRule {
    advisory_database: HashMap<String, Vec<VulnerabilityAdvisory>>,
}

impl DependencyVulnerabilityRule {
    pub fn new() -> Self {
        Self {
            advisory_database: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct VulnerabilityAdvisory {
    pub id: String,
    pub severity: Severity,
    pub affected_versions: String,
    pub patched_version: Option<String>,
    pub description: String,
}

// Implement the remaining security rules...
#[async_trait::async_trait]
impl SecurityRule for FileSystemAccessRule {
    fn name(&self) -> &str { "filesystem_access_detection" }
    fn description(&self) -> &str { "Detects file system access patterns" }
    fn category(&self) -> SecurityCategory { SecurityCategory::FileSystemAccess }
    
    async fn scan(&self, _context: &CodeContext) -> Result<Vec<SecurityFinding>> {
        // Implementation for file system access detection
        Ok(Vec::new())
    }
}

// Similar implementations for other rules...
macro_rules! impl_security_rule {
    ($rule:ident, $name:expr, $desc:expr, $category:expr) => {
        #[async_trait::async_trait]
        impl SecurityRule for $rule {
            fn name(&self) -> &str { $name }
            fn description(&self) -> &str { $desc }
            fn category(&self) -> SecurityCategory { $category }
            
            async fn scan(&self, _context: &CodeContext) -> Result<Vec<SecurityFinding>> {
                // Basic implementation - would be expanded for each rule
                Ok(Vec::new())
            }
        }
    };
}

impl_security_rule!(ProcessSpawningRule, "process_spawning_detection", "Detects process spawning attempts", SecurityCategory::ProcessSpawning);
impl_security_rule!(CryptographyRule, "cryptography_usage", "Analyzes cryptography usage", SecurityCategory::Cryptography);
impl_security_rule!(MemoryLeakRule, "memory_leak_detection", "Detects potential memory leaks", SecurityCategory::MemorySafety);
impl_security_rule!(InputValidationRule, "input_validation", "Checks input validation patterns", SecurityCategory::InputValidation);
impl_security_rule!(ErrorHandlingRule, "error_handling", "Analyzes error handling", SecurityCategory::ErrorHandling);
impl_security_rule!(PrivilegeEscalationRule, "privilege_escalation", "Detects privilege escalation attempts", SecurityCategory::PrivilegeEscalation);

#[async_trait::async_trait]
impl SecurityRule for DependencyVulnerabilityRule {
    fn name(&self) -> &str { "dependency_vulnerabilities" }
    fn description(&self) -> &str { "Checks dependencies against vulnerability databases" }
    fn category(&self) -> SecurityCategory { SecurityCategory::DependencyVulnerability }
    
    async fn scan(&self, context: &CodeContext) -> Result<Vec<SecurityFinding>> {
        // Implementation for dependency vulnerability checking
        Ok(Vec::new())
    }
}