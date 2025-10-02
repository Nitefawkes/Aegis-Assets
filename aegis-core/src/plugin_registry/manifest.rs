//! Plugin manifest parser and validator
//!
//! Parses and validates plugin.toml files according to the Aegis plugin specification.

use anyhow::{Result, Context, bail, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use toml;
use tracing::{info, warn, debug, error};

use super::models::*;

/// Plugin manifest parser
pub struct ManifestParser;

impl ManifestParser {
    /// Parse a plugin.toml file from a string
    pub fn parse_from_str(content: &str) -> Result<PluginManifest> {
        debug!("Parsing plugin manifest from string");

        let manifest: PluginManifestToml = toml::from_str(content)
            .context("Failed to parse plugin.toml content")?;

        Self::validate_and_convert(manifest)
    }

    /// Parse a plugin.toml file from a file path
    pub fn parse_from_file<P: AsRef<Path>>(path: P) -> Result<PluginManifest> {
        debug!("Parsing plugin manifest from file: {}", path.as_ref().display());

        let content = std::fs::read_to_string(path)
            .context("Failed to read plugin.toml file")?;

        Self::parse_from_str(&content)
    }

    /// Validate and convert TOML manifest to internal representation
    fn validate_and_convert(manifest: PluginManifestToml) -> Result<PluginManifest> {
        info!("Validating plugin manifest");

        // Validate package section
        Self::validate_package(&manifest.package)?;
        Self::validate_plugin(&manifest.plugin)?;
        Self::validate_compliance(&manifest.compliance)?;

        // Convert to internal representation
        let converted = PluginManifest {
            package: manifest.package.into(),
            plugin: manifest.plugin.into(),
            compliance: manifest.compliance.into(),
            dependencies: manifest.dependencies.unwrap_or_default(),
            dev_dependencies: manifest.dev_dependencies.unwrap_or_default(),
            build: manifest.build.map(Into::into),
            testing: manifest.testing.map(Into::into),
            security: manifest.security.map(Into::into),
        };

        debug!("Plugin manifest validation completed successfully");
        Ok(converted)
    }

    /// Validate package section
    fn validate_package(package: &PackageInfoToml) -> Result<()> {
        if package.name.is_empty() {
            bail!("Package name cannot be empty");
        }

        if package.version.is_empty() {
            bail!("Package version cannot be empty");
        }

        if package.authors.is_empty() {
            bail!("Package must have at least one author");
        }

        if package.license.is_empty() {
            bail!("Package license cannot be empty");
        }

        // Validate version format
        if !Self::is_valid_semver(&package.version) {
            bail!("Package version '{}' must follow semantic versioning (e.g., 1.0.0)", package.version);
        }

        // Validate email format for authors
        for author in &package.authors {
            if !author.contains('@') && !author.contains('<') {
                warn!("Author '{}' does not appear to be a valid email address", author);
            }
        }

        Ok(())
    }

    /// Validate plugin section
    fn validate_plugin(plugin: &PluginInfoToml) -> Result<()> {
        if plugin.aegis_version.is_empty() {
            bail!("Plugin must specify compatible Aegis version");
        }

        if plugin.plugin_api_version.is_empty() {
            bail!("Plugin must specify plugin API version");
        }

        if plugin.format_support.is_empty() {
            bail!("Plugin must support at least one file format");
        }

        // Validate Aegis version requirement
        if !Self::is_valid_version_requirement(&plugin.aegis_version) {
            bail!("Invalid Aegis version requirement: {}", plugin.aegis_version);
        }

        // Validate plugin API version
        if !Self::is_valid_version_requirement(&plugin.plugin_api_version) {
            bail!("Invalid plugin API version requirement: {}", plugin.plugin_api_version);
        }

        // Validate format support
        for (i, format) in plugin.format_support.iter().enumerate() {
            if format.extension.is_empty() {
                bail!("Format {}: extension cannot be empty", i + 1);
            }

            if format.description.is_empty() {
                bail!("Format {}: description cannot be empty", i + 1);
            }

            if !format.extension.chars().all(|c| c.is_alphanumeric() || c == '_') {
                bail!("Format {}: extension '{}' contains invalid characters (only alphanumeric and underscore allowed)",
                      i + 1, format.extension);
            }
        }

        Ok(())
    }

    /// Validate compliance section
    fn validate_compliance(compliance: &ComplianceInfoToml) -> Result<()> {
        // Risk level validation
        match compliance.risk_level.as_str() {
            "low" | "medium" | "high" => {},
            _ => bail!("Invalid risk level: {} (must be low, medium, or high)", compliance.risk_level),
        }

        // Publisher policy validation
        match compliance.publisher_policy.as_str() {
            "permissive" | "mixed" | "aggressive" | "unknown" => {},
            _ => bail!("Invalid publisher policy: {} (must be permissive, mixed, aggressive, or unknown)",
                       compliance.publisher_policy),
        }

        Ok(())
    }

    /// Check if a version string follows semantic versioning
    fn is_valid_semver(version: &str) -> bool {
        // Basic semantic versioning regex: MAJOR.MINOR.PATCH(-PRERELEASE)(+BUILD)
        let semver_regex = regex::Regex::new(r"^(\d+)\.(\d+)\.(\d+)(?:-([a-zA-Z0-9\-\.]+))?(?:\+([a-zA-Z0-9\-\.]+))?$")
            .expect("Invalid semver regex");

        semver_regex.is_match(version)
    }

    /// Check if a version requirement is valid (supports common patterns)
    fn is_valid_version_requirement(requirement: &str) -> bool {
        if requirement.is_empty() {
            return false;
        }

        // Allow common patterns: ^1.0.0, ~1.0.0, >=1.0.0, 1.0.0, *
        let patterns = [
            r"^\^(\d+\.)?(\d+\.)?(\*|\d+)$",     // ^1.0.0, ^1.0, ^1
            r"^~(\d+\.)?(\d+\.)?(\*|\d+)$",     // ~1.0.0, ~1.0, ~1
            r"^>=?\d+\.\d+\.\d+$",               // >=1.0.0, 1.0.0
            r"^<\d+\.\d+\.\d+$",                // <1.0.0
            r"^\d+\.\d+\.\d+$",                 // 1.0.0
            r"^\*$",                            // *
            r"^x\.x\.x$",                       // x.x.x
        ];

        patterns.iter().any(|pattern| {
            regex::Regex::new(pattern).unwrap().is_match(requirement)
        })
    }

    /// Create a sample plugin.toml file
    pub fn create_sample_manifest() -> String {
        r#"[package]
name = "unity-asset-extractor"
version = "1.0.0"
description = "Extract assets from Unity game files"
authors = ["developer@example.com"]
license = "MIT"
homepage = "https://github.com/example/unity-asset-extractor"
repository = "https://github.com/example/unity-asset-extractor"
keywords = ["unity", "assets", "extraction", "game"]

[plugin]
aegis_version = "^0.2.0"
plugin_api_version = "^1.0.0"
format_support = [
    { extension = "unity3d", description = "Unity asset bundle", tested = true },
    { extension = "prefab", description = "Unity prefab file", tested = true },
    { extension = "unitypackage", description = "Unity package", tested = false }
]
features = []

[compliance]
risk_level = "low"
publisher_policy = "permissive"
bounty_eligible = true
enterprise_approved = false

# Optional: Dependencies
[dependencies]
serde = "^1.0.0"
tokio = "^1.0.0"

# Optional: Development dependencies
[dev_dependencies]
tempfile = "^3.0.0"

# Optional: Build configuration
[build]
min_rust_version = "1.70.0"
features = ["default"]
targets = ["x86_64-unknown-linux-gnu"]

# Optional: Testing configuration
[testing]
sample_files = ["tests/samples/test.unity3d"]
required_success_rate = 0.95

# Optional: Security configuration
[security]
sandbox_permissions = ["FileRead", "FileWriteTemp"]
code_signing_cert = "Community"
vulnerability_scan = "Required"
"#.to_string()
    }
}

/// TOML representation of plugin manifest (serde compatible)
#[derive(Debug, Deserialize, Serialize)]
struct PluginManifestToml {
    #[serde(rename = "package")]
    pub package: PackageInfoToml,

    #[serde(rename = "plugin")]
    pub plugin: PluginInfoToml,

    #[serde(rename = "compliance")]
    pub compliance: ComplianceInfoToml,

    #[serde(rename = "dependencies")]
    pub dependencies: Option<HashMap<String, String>>,

    #[serde(rename = "dev_dependencies")]
    pub dev_dependencies: Option<HashMap<String, String>>,

    #[serde(rename = "build")]
    pub build: Option<BuildConfigToml>,

    #[serde(rename = "testing")]
    pub testing: Option<TestingConfigToml>,

    #[serde(rename = "security")]
    pub security: Option<SecurityConfigToml>,
}

/// TOML package section
#[derive(Debug, Deserialize, Serialize)]
struct PackageInfoToml {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub authors: Vec<String>,
    pub license: String,
    pub homepage: Option<String>,
    pub repository: Option<String>,
    pub keywords: Vec<String>,
}

impl From<PackageInfoToml> for PackageInfo {
    fn from(toml: PackageInfoToml) -> Self {
        Self {
            name: toml.name,
            version: toml.version,
            description: toml.description,
            authors: toml.authors,
            license: toml.license,
            homepage: toml.homepage,
            repository: toml.repository,
            keywords: toml.keywords,
        }
    }
}

/// TOML plugin section
#[derive(Debug, Deserialize, Serialize)]
struct PluginInfoToml {
    pub aegis_version: String,
    pub plugin_api_version: String,
    #[serde(rename = "format_support")]
    pub format_support: Vec<FileFormatInfoToml>,
    pub features: Vec<String>,
}

impl From<PluginInfoToml> for PluginInfo {
    fn from(toml: PluginInfoToml) -> Self {
        Self {
            aegis_version: toml.aegis_version,
            plugin_api_version: toml.plugin_api_version,
            engine_name: None, // Optional field not in TOML
            format_support: toml.format_support.into_iter().map(Into::into).collect(),
            features: toml.features,
        }
    }
}

/// TOML file format info
#[derive(Debug, Deserialize, Serialize)]
struct FileFormatInfoToml {
    pub extension: String,
    pub description: String,
    pub mime_type: Option<String>,
    pub tested: bool,
}

impl From<FileFormatInfoToml> for FileFormatInfo {
    fn from(toml: FileFormatInfoToml) -> Self {
        Self {
            extension: toml.extension,
            description: toml.description,
            mime_type: toml.mime_type,
            tested: toml.tested,
        }
    }
}

/// TOML compliance section
#[derive(Debug, Deserialize, Serialize)]
struct ComplianceInfoToml {
    pub risk_level: String,
    pub publisher_policy: String,
    pub bounty_eligible: bool,
    pub enterprise_approved: bool,
    pub notes: Option<String>,
}

impl From<ComplianceInfoToml> for ComplianceInfo {
    fn from(toml: ComplianceInfoToml) -> Self {
        Self {
            risk_level: match toml.risk_level.as_str() {
                "low" => RiskLevel::Low,
                "medium" => RiskLevel::Medium,
                "high" => RiskLevel::High,
                _ => RiskLevel::Low, // Default fallback
            },
            publisher_policy: match toml.publisher_policy.as_str() {
                "permissive" => PublisherPolicy::Permissive,
                "mixed" => PublisherPolicy::Mixed,
                "aggressive" => PublisherPolicy::Aggressive,
                "unknown" => PublisherPolicy::Unknown,
                _ => PublisherPolicy::Unknown, // Default fallback
            },
            bounty_eligible: toml.bounty_eligible,
            enterprise_approved: toml.enterprise_approved,
            notes: toml.notes,
        }
    }
}

/// TOML build configuration
#[derive(Debug, Deserialize, Serialize)]
struct BuildConfigToml {
    pub min_rust_version: Option<String>,
    pub features: Vec<String>,
    pub targets: Vec<String>,
}

impl From<BuildConfigToml> for BuildConfig {
    fn from(toml: BuildConfigToml) -> Self {
        Self {
            min_rust_version: toml.min_rust_version,
            features: toml.features,
            targets: toml.targets,
        }
    }
}

/// TOML testing configuration
#[derive(Debug, Deserialize, Serialize)]
struct TestingConfigToml {
    pub sample_files: Vec<String>,
    pub required_success_rate: f64,
    pub performance_benchmark: Option<PerformanceBenchmarkToml>,
}

/// TOML performance benchmark
#[derive(Debug, Deserialize, Serialize)]
struct PerformanceBenchmarkToml {
    pub max_extraction_time: String,
    pub max_memory_usage: String,
}

impl From<TestingConfigToml> for TestingConfig {
    fn from(toml: TestingConfigToml) -> Self {
        Self {
            sample_files: toml.sample_files,
            required_success_rate: toml.required_success_rate,
            performance_benchmark: toml.performance_benchmark.map(Into::into),
        }
    }
}

impl From<PerformanceBenchmarkToml> for PerformanceBenchmark {
    fn from(toml: PerformanceBenchmarkToml) -> Self {
        Self {
            max_extraction_time: toml.max_extraction_time,
            max_memory_usage: toml.max_memory_usage,
        }
    }
}

/// TOML security configuration
#[derive(Debug, Deserialize, Serialize)]
struct SecurityConfigToml {
    pub sandbox_permissions: Vec<String>,
    pub code_signing_cert: String,
    pub vulnerability_scan: String,
}

impl From<SecurityConfigToml> for SecurityConfig {
    fn from(toml: SecurityConfigToml) -> Self {
        Self {
            sandbox_permissions: toml.sandbox_permissions.into_iter().map(|s| {
                match s.as_str() {
                    "FileRead" => SandboxPermission::FileRead,
                    "FileWriteTemp" => SandboxPermission::FileWriteTemp,
                    "ProcessSpawnNone" => SandboxPermission::ProcessSpawnNone,
                    "NetworkNone" => SandboxPermission::NetworkNone,
                    "FilesystemRestricted" => SandboxPermission::FilesystemRestricted,
                    _ => SandboxPermission::FileRead, // Default fallback
                }
            }).collect(),
            code_signing_cert: match toml.code_signing_cert.as_str() {
                "Community" => CodeSigningLevel::Community,
                "Verified" => CodeSigningLevel::Verified,
                "Enterprise" => CodeSigningLevel::Enterprise,
                _ => CodeSigningLevel::Community, // Default fallback
            },
            vulnerability_scan: match toml.vulnerability_scan.as_str() {
                "Required" => ScanRequirement::Required,
                "Optional" => ScanRequirement::Optional,
                "Disabled" => ScanRequirement::Disabled,
                _ => ScanRequirement::Optional, // Default fallback
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_manifest() {
        let manifest_content = r#"
[package]
name = "test-plugin"
version = "1.0.0"
description = "A test plugin"
authors = ["test@example.com"]
license = "MIT"
keywords = ["test"]

[plugin]
aegis_version = "^0.2.0"
plugin_api_version = "^1.0.0"
format_support = [
    { extension = "test", description = "Test format", tested = true }
]

[compliance]
risk_level = "low"
publisher_policy = "permissive"
bounty_eligible = true
enterprise_approved = false
        "#;

        let manifest = ManifestParser::parse_from_str(manifest_content);
        assert!(manifest.is_ok());

        let manifest = manifest.unwrap();
        assert_eq!(manifest.package.name, "test-plugin");
        assert_eq!(manifest.package.version, "1.0.0");
        assert_eq!(manifest.plugin.format_support.len(), 1);
        assert!(matches!(manifest.compliance.risk_level, RiskLevel::Low));
    }

    #[test]
    fn test_parse_invalid_manifest_missing_name() {
        let manifest_content = r#"
[package]
version = "1.0.0"
description = "A test plugin"
authors = ["test@example.com"]
license = "MIT"

[plugin]
aegis_version = "^0.2.0"
plugin_api_version = "^1.0.0"
format_support = []

[compliance]
risk_level = "low"
publisher_policy = "permissive"
bounty_eligible = true
enterprise_approved = false
        "#;

        let manifest = ManifestParser::parse_from_str(manifest_content);
        assert!(manifest.is_err());
        assert!(manifest.unwrap_err().to_string().contains("Package name cannot be empty"));
    }

    #[test]
    fn test_parse_invalid_manifest_bad_version() {
        let manifest_content = r#"
[package]
name = "test-plugin"
version = "invalid-version"
description = "A test plugin"
authors = ["test@example.com"]
license = "MIT"

[plugin]
aegis_version = "^0.2.0"
plugin_api_version = "^1.0.0"
format_support = []

[compliance]
risk_level = "low"
publisher_policy = "permissive"
bounty_eligible = true
enterprise_approved = false
        "#;

        let manifest = ManifestParser::parse_from_str(manifest_content);
        assert!(manifest.is_err());
        assert!(manifest.unwrap_err().to_string().contains("semantic versioning"));
    }

    #[test]
    fn test_version_validation() {
        assert!(ManifestParser::is_valid_semver("1.0.0"));
        assert!(ManifestParser::is_valid_semver("1.0.0-alpha.1"));
        assert!(ManifestParser::is_valid_semver("2.0.0+build.1"));

        assert!(!ManifestParser::is_valid_semver(""));
        assert!(!ManifestParser::is_valid_semver("1.0"));
        assert!(!ManifestParser::is_valid_semver("invalid"));
    }

    #[test]
    fn test_version_requirement_validation() {
        assert!(ManifestParser::is_valid_version_requirement("^1.0.0"));
        assert!(ManifestParser::is_valid_version_requirement("~1.0.0"));
        assert!(ManifestParser::is_valid_version_requirement(">=1.0.0"));
        assert!(ManifestParser::is_valid_version_requirement("1.0.0"));
        assert!(ManifestParser::is_valid_version_requirement("*"));

        assert!(!ManifestParser::is_valid_version_requirement(""));
        assert!(!ManifestParser::is_valid_version_requirement("invalid"));
    }

    #[test]
    fn test_create_sample_manifest() {
        let sample = ManifestParser::create_sample_manifest();
        assert!(sample.contains("[package]"));
        assert!(sample.contains("[plugin]"));
        assert!(sample.contains("[compliance]"));
        assert!(sample.contains("format_support"));
    }
}
