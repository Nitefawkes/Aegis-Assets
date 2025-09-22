use crate::archive::{ComplianceLevel, ComplianceProfile, FormatSupport};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;
use tracing::{debug, info, warn};

/// Compliance checker for validating extraction operations
pub struct ComplianceChecker {
    profiles: HashMap<String, ComplianceProfile>,
    strict_mode: bool,
}

impl ComplianceChecker {
    /// Create a new compliance checker
    pub fn new() -> Self {
        Self {
            profiles: HashMap::new(),
            strict_mode: false,
        }
    }

    /// Enable strict compliance mode (block all high-risk operations)
    pub fn with_strict_mode(mut self) -> Self {
        self.strict_mode = true;
        self
    }

    /// Load compliance profiles from directory
    pub fn load_profiles(&mut self, profiles_dir: &Path) -> Result<usize> {
        debug!(
            "Loading compliance profiles from: {}",
            profiles_dir.display()
        );

        if !profiles_dir.exists() {
            warn!(
                "Compliance profiles directory does not exist: {}",
                profiles_dir.display()
            );
            return Ok(0);
        }

        let mut loaded_count = 0;

        for entry in std::fs::read_dir(profiles_dir).context("Failed to read profiles directory")? {
            let entry = entry?;
            let path = entry.path();

            if path
                .extension()
                .map_or(false, |ext| ext == "yaml" || ext == "yml")
            {
                match self.load_single_profile(&path) {
                    Ok(profile) => {
                        let key = path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("unknown")
                            .to_string();

                        info!("Loaded compliance profile: {} ({})", key, profile.publisher);
                        self.profiles.insert(key, profile);
                        loaded_count += 1;
                    }
                    Err(e) => {
                        warn!("Failed to load profile {}: {}", path.display(), e);
                    }
                }
            }
        }

        info!("Loaded {} compliance profiles", loaded_count);
        Ok(loaded_count)
    }

    /// Load a single compliance profile
    fn load_single_profile(&self, path: &Path) -> Result<ComplianceProfile> {
        let content = std::fs::read_to_string(path).context("Failed to read profile file")?;

        serde_yaml::from_str(&content).context("Failed to parse profile YAML")
    }

    /// Check if extraction is allowed for a given game/file
    pub fn check_extraction_allowed(&self, game_id: &str, format: &str) -> ComplianceResult {
        let profile = self.get_profile_for_game(game_id);
        let (extra_warnings, extra_recommendations) =
            match self.check_format_support(game_id, format) {
                FormatSupportResult::Supported => (Vec::new(), Vec::new()),
                FormatSupportResult::CommunityOnly { warning } => (
                    vec![warning],
                    vec!["Ensure community plugins comply with publisher guidelines.".to_string()],
                ),
                FormatSupportResult::Unknown { recommendation } => (
                    vec!["Format support is unknown for this title.".to_string()],
                    vec![recommendation],
                ),
                FormatSupportResult::NotSupported { reason } => {
                    return ComplianceResult::Blocked {
                        profile: profile.clone(),
                        reason,
                        alternatives: vec![
                            "Consider requesting permission from the publisher.".to_string(),
                            "Review the publisher's modding or asset usage policy.".to_string(),
                        ],
                    };
                }
            };

        match profile.enforcement_level {
            ComplianceLevel::Permissive => {
                let mut recommendations = vec![
                    "This publisher generally allows modding and asset extraction.".to_string(),
                ];
                recommendations.extend(extra_recommendations.clone());

                ComplianceResult::Allowed {
                    profile: profile.clone(),
                    warnings: extra_warnings.clone(),
                    recommendations,
                }
            }

            ComplianceLevel::Neutral => {
                let mut warnings = vec![
                    "Publisher has no explicit modding policy. Proceed with caution.".to_string(),
                ];

                warnings.extend(extra_warnings.clone());

                let mut recommendations = vec![
                    "Ensure you own the original game files.".to_string(),
                    "Use extracted assets only for personal projects.".to_string(),
                    "Do not redistribute extracted assets.".to_string(),
                ];

                recommendations.extend(extra_recommendations.clone());

                ComplianceResult::AllowedWithWarnings {
                    profile: profile.clone(),
                    warnings,
                    recommendations,
                }
            }

            ComplianceLevel::HighRisk => {
                if self.strict_mode {
                    ComplianceResult::Blocked {
                        profile: profile.clone(),
                        reason: "High-risk publisher in strict mode".to_string(),
                        alternatives: vec![
                            "Check if official modding tools are available".to_string(),
                            "Look for community-approved alternatives".to_string(),
                        ],
                    }
                } else {
                    let mut warnings = vec![
                        "WARNING: This publisher actively enforces IP rights.".to_string(),
                        "Extraction may violate terms of service.".to_string(),
                        "Use at your own risk.".to_string(),
                    ];

                    warnings.extend(extra_warnings);

                    ComplianceResult::HighRiskWarning {
                        profile: profile.clone(),
                        warnings,
                        explicit_consent_required: true,
                    }
                }
            }
        }
    }

    /// Get compliance profile for a game, with fallback to default
    fn get_profile_for_game(&self, game_id: &str) -> &ComplianceProfile {
        // Try exact match first
        if let Some(profile) = self.profiles.get(game_id) {
            return profile;
        }

        // Try publisher-level match (extract publisher from game_id)
        let publisher_key = game_id.split('_').next().unwrap_or(game_id);
        if let Some(profile) = self.profiles.get(publisher_key) {
            return profile;
        }

        // Fallback to default neutral profile
        static DEFAULT_PROFILE: std::sync::OnceLock<ComplianceProfile> = std::sync::OnceLock::new();
        DEFAULT_PROFILE.get_or_init(|| ComplianceProfile {
            publisher: "Unknown".to_string(),
            game_id: None,
            enforcement_level: ComplianceLevel::Neutral,
            official_support: false,
            bounty_eligible: false,
            enterprise_warning: Some("Unknown publisher. Compliance status unclear.".to_string()),
            mod_policy_url: None,
            supported_formats: HashMap::new(),
        })
    }

    /// Validate format support for a specific publisher
    pub fn check_format_support(&self, game_id: &str, format: &str) -> FormatSupportResult {
        let profile = self.get_profile_for_game(game_id);

        match profile.supported_formats.get(format) {
            Some(FormatSupport::Supported) => FormatSupportResult::Supported,
            Some(FormatSupport::CommunityOnly) => FormatSupportResult::CommunityOnly {
                warning: "This format is supported by community plugins only".to_string(),
            },
            Some(FormatSupport::NotSupported) => FormatSupportResult::NotSupported {
                reason: "Publisher has explicitly blocked this format".to_string(),
            },
            None => FormatSupportResult::Unknown {
                recommendation: "Format support unclear - proceed with caution".to_string(),
            },
        }
    }

    /// Generate compliance report for enterprise use
    pub fn generate_compliance_report(&self, extractions: &[ComplianceCheck]) -> ComplianceReport {
        let mut allowed = 0;
        let mut warned = 0;
        let mut blocked = 0;
        let mut high_risk = 0;

        let mut issues = Vec::new();

        for check in extractions {
            match &check.result {
                ComplianceResult::Allowed { .. } => allowed += 1,
                ComplianceResult::AllowedWithWarnings { warnings, .. } => {
                    warned += 1;
                    for warning in warnings {
                        issues.push(ComplianceIssue {
                            game_id: check.game_id.clone(),
                            issue_type: IssueType::Warning,
                            message: warning.clone(),
                        });
                    }
                }
                ComplianceResult::HighRiskWarning { warnings, .. } => {
                    high_risk += 1;
                    for warning in warnings {
                        issues.push(ComplianceIssue {
                            game_id: check.game_id.clone(),
                            issue_type: IssueType::HighRisk,
                            message: warning.clone(),
                        });
                    }
                }
                ComplianceResult::Blocked { reason, .. } => {
                    blocked += 1;
                    issues.push(ComplianceIssue {
                        game_id: check.game_id.clone(),
                        issue_type: IssueType::Blocked,
                        message: reason.clone(),
                    });
                }
            }
        }

        let risk_score = calculate_risk_score(allowed, warned, high_risk, blocked);

        let recommendations = generate_recommendations(&issues);

        ComplianceReport {
            summary: ComplianceSummary {
                total_checks: extractions.len(),
                allowed,
                warned,
                high_risk,
                blocked,
                risk_score,
            },
            issues,
            recommendations,
            generated_at: chrono::Utc::now(),
        }
    }
}

/// Result of a compliance check
#[derive(Debug, Clone)]
pub enum ComplianceResult {
    /// Extraction is allowed without restrictions
    Allowed {
        profile: ComplianceProfile,
        warnings: Vec<String>,
        recommendations: Vec<String>,
    },

    /// Extraction allowed but with warnings
    AllowedWithWarnings {
        profile: ComplianceProfile,
        warnings: Vec<String>,
        recommendations: Vec<String>,
    },

    /// High risk - user must explicitly consent
    HighRiskWarning {
        profile: ComplianceProfile,
        warnings: Vec<String>,
        explicit_consent_required: bool,
    },

    /// Extraction blocked (strict mode or explicit prohibition)
    Blocked {
        profile: ComplianceProfile,
        reason: String,
        alternatives: Vec<String>,
    },
}

/// Result of format support check
#[derive(Debug, Clone)]
pub enum FormatSupportResult {
    /// Format is officially supported
    Supported,

    /// Format supported by community plugins only
    CommunityOnly { warning: String },

    /// Format explicitly not supported
    NotSupported { reason: String },

    /// Format support status unknown
    Unknown { recommendation: String },
}

/// Individual compliance check record
#[derive(Debug, Clone)]
pub struct ComplianceCheck {
    pub game_id: String,
    pub format: String,
    pub result: ComplianceResult,
    pub checked_at: chrono::DateTime<chrono::Utc>,
}

/// Compliance report for enterprise auditing
#[derive(Debug, Clone)]
pub struct ComplianceReport {
    pub summary: ComplianceSummary,
    pub issues: Vec<ComplianceIssue>,
    pub recommendations: Vec<String>,
    pub generated_at: chrono::DateTime<chrono::Utc>,
}

/// Summary statistics for compliance report
#[derive(Debug, Clone)]
pub struct ComplianceSummary {
    pub total_checks: usize,
    pub allowed: usize,
    pub warned: usize,
    pub high_risk: usize,
    pub blocked: usize,
    pub risk_score: f64, // 0.0 (low risk) to 1.0 (high risk)
}

/// Individual compliance issue
#[derive(Debug, Clone)]
pub struct ComplianceIssue {
    pub game_id: String,
    pub issue_type: IssueType,
    pub message: String,
}

/// Types of compliance issues
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IssueType {
    Warning,
    HighRisk,
    Blocked,
}

/// Calculate risk score based on compliance results
fn calculate_risk_score(allowed: usize, warned: usize, high_risk: usize, blocked: usize) -> f64 {
    let total = allowed + warned + high_risk + blocked;
    if total == 0 {
        return 0.0;
    }

    let risk_weighted = (warned as f64 * 0.25) + (high_risk as f64 * 0.75) + (blocked as f64 * 1.0);

    risk_weighted / total as f64
}

/// Generate recommendations based on compliance issues
fn generate_recommendations(issues: &[ComplianceIssue]) -> Vec<String> {
    let mut recommendations = Vec::new();

    let high_risk_count = issues
        .iter()
        .filter(|i| i.issue_type == IssueType::HighRisk)
        .count();

    let blocked_count = issues
        .iter()
        .filter(|i| i.issue_type == IssueType::Blocked)
        .count();

    if blocked_count > 0 {
        recommendations.push(
            "Some extractions are blocked due to compliance violations. Consider using official modding tools where available.".to_string()
        );
    }

    if high_risk_count > 0 {
        recommendations.push(
            "High-risk extractions detected. Ensure all extracted content is used only for personal, non-commercial purposes.".to_string()
        );

        recommendations.push(
            "Consider enabling strict compliance mode to automatically block high-risk operations."
                .to_string(),
        );
    }

    recommendations.push(
        "Regularly update compliance profiles to reflect changes in publisher policies."
            .to_string(),
    );

    recommendations
        .push("Maintain audit logs of all extraction activities for legal compliance.".to_string());

    recommendations
}

impl Default for ComplianceChecker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_compliance_checker_creation() {
        let checker = ComplianceChecker::new();
        assert!(!checker.strict_mode);
        assert!(checker.profiles.is_empty());
    }

    #[test]
    fn test_strict_mode() {
        let checker = ComplianceChecker::new().with_strict_mode();
        assert!(checker.strict_mode);
    }

    #[test]
    fn test_default_compliance_check() {
        let checker = ComplianceChecker::new();
        let result = checker.check_extraction_allowed("unknown_game", "unknown_format");

        // Should get neutral compliance result for unknown games
        match result {
            ComplianceResult::AllowedWithWarnings { profile, .. } => {
                assert_eq!(profile.enforcement_level, ComplianceLevel::Neutral);
            }
            _ => panic!("Expected AllowedWithWarnings for unknown game"),
        }
    }

    #[test]
    fn test_risk_score_calculation() {
        assert_eq!(calculate_risk_score(4, 0, 0, 0), 0.0); // All allowed
        assert_eq!(calculate_risk_score(0, 0, 0, 4), 1.0); // All blocked
        assert_eq!(calculate_risk_score(2, 2, 0, 0), 0.125); // Half warned
    }

    #[test]
    fn test_profile_loading() {
        let temp_dir = TempDir::new().unwrap();
        let profiles_dir = temp_dir.path().join("profiles");
        fs::create_dir(&profiles_dir).unwrap();

        // Create a test profile
        let test_profile = r#"
publisher: "Test Publisher"
enforcement_level: "Permissive" 
official_support: true
bounty_eligible: true
supported_formats:
  unity: "Supported"
  unreal: "CommunityOnly"
"#;

        fs::write(profiles_dir.join("test.yaml"), test_profile).unwrap();

        let mut checker = ComplianceChecker::new();
        let loaded = checker.load_profiles(&profiles_dir).unwrap();

        assert_eq!(loaded, 1);
        assert!(checker.profiles.contains_key("test"));
    }
}
