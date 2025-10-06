//! # Aegis Security Framework
//! 
//! Comprehensive security scanning, compliance verification, and threat detection
//! for the Aegis-Assets plugin ecosystem.
//! 
//! ## Features
//! 
//! - **Static Code Analysis**: Automated vulnerability detection
//! - **Code Signing**: Ed25519-based plugin verification
//! - **Runtime Sandboxing**: Process isolation and monitoring
//! - **Compliance Automation**: Policy enforcement and reporting
//! - **Threat Intelligence**: Malware detection and analysis
//! - **Enterprise Audit**: Comprehensive logging and trails

pub mod scanning;
pub mod signing;
pub mod sandbox;
pub mod compliance;
pub mod threats;
pub mod audit;
pub mod policy;

// Re-export main types
pub use scanning::{SecurityScanner, ScanReport, SecurityFinding, Severity};
pub use signing::{CodeSigner, SignatureVerifier, PluginSignature};
pub use sandbox::{PluginSandbox, SandboxConfig, SandboxedExecution};
pub use compliance::{ComplianceVerifier, ComplianceResult, PolicyViolation};
pub use threats::{ThreatDetector, ThreatReport, ThreatIndicator};
pub use audit::{AuditLogger, SecurityEvent, AuditTrail};
pub use policy::{SecurityPolicy, PolicyEngine, PolicyEvaluationResult};

use anyhow::Result;
use serde::Serialize;
use std::sync::Arc;
use tracing::{info, warn};

/// Central security coordinator for all security operations
pub struct SecurityController {
    scanner: Arc<SecurityScanner>,
    verifier: Arc<SignatureVerifier>,
    sandbox: Arc<PluginSandbox>,
    compliance: Arc<ComplianceVerifier>,
    threat_detector: Arc<ThreatDetector>,
    audit_logger: Arc<AuditLogger>,
    policy_engine: Arc<PolicyEngine>,
}

impl SecurityController {
    /// Create a new security controller with default configuration
    pub async fn new() -> Result<Self> {
        info!("Initializing Aegis Security Framework");
        
        let scanner = Arc::new(SecurityScanner::new().await?);
        let verifier = Arc::new(SignatureVerifier::new()?);
        let sandbox = Arc::new(PluginSandbox::new(SandboxConfig::default())?);
        let compliance = Arc::new(ComplianceVerifier::new()?);
        let threat_detector = Arc::new(ThreatDetector::new().await?);
        let audit_logger = Arc::new(AuditLogger::new().await?);
        let policy_engine = Arc::new(PolicyEngine::new()?);
        
        Ok(Self {
            scanner,
            verifier,
            sandbox,
            compliance,
            threat_detector,
            audit_logger,
            policy_engine,
        })
    }
    
    /// Perform comprehensive security evaluation of a plugin
    pub async fn evaluate_plugin(&self, plugin_path: &std::path::Path) -> Result<SecurityEvaluation> {
        let evaluation_id = uuid::Uuid::new_v4();
        info!("Starting security evaluation: {}", evaluation_id);
        
        // 1. Signature verification
        let signature_result = self.verifier.verify_plugin(plugin_path).await?;
        
        // 2. Static code analysis
        let scan_report = self.scanner.scan_plugin(plugin_path).await?;
        
        // 3. Threat detection
        let threat_report = self.threat_detector.analyze_plugin(plugin_path).await?;
        
        // 4. Compliance verification
        let compliance_result = self.compliance.verify_plugin(plugin_path, &scan_report).await?;
        
        // 5. Policy evaluation
        let policy_result = self.policy_engine.evaluate_plugin(plugin_path, &scan_report).await?;
        
        // 6. Calculate overall security score
        let security_score = self.calculate_security_score(
            &signature_result,
            &scan_report, 
            &threat_report,
            &compliance_result,
            &policy_result
        );
        
        let evaluation = SecurityEvaluation {
            id: evaluation_id,
            plugin_path: plugin_path.to_path_buf(),
            signature_verification: signature_result,
            static_analysis: scan_report,
            threat_analysis: threat_report,
            compliance_verification: compliance_result,
            policy_evaluation: policy_result,
            security_score,
            timestamp: chrono::Utc::now(),
        };
        
        // 7. Log security event
        self.audit_logger.log_security_evaluation(&evaluation).await?;
        
        info!("Security evaluation complete: {} (score: {})", 
               evaluation_id, evaluation.security_score.overall_score);
        
        Ok(evaluation)
    }
    
    /// Execute a plugin in a sandboxed environment
    pub async fn execute_plugin_safely(
        &self,
        plugin_path: &std::path::Path,
        args: &[String]
    ) -> Result<SandboxedExecution> {
        // Pre-execution security check
        let evaluation = self.evaluate_plugin(plugin_path).await?;
        
        if evaluation.security_score.overall_score < 50 {
            warn!("Plugin failed security evaluation, blocking execution");
            return Err(anyhow::anyhow!("Plugin security score too low for execution"));
        }
        
        // Execute in sandbox
        let execution = self.sandbox.execute_plugin(plugin_path, args).await?;
        
        // Log execution event
        self.audit_logger.log_plugin_execution(plugin_path, &execution).await?;
        
        Ok(execution)
    }
    
    fn calculate_security_score(
        &self,
        signature: &signing::VerificationResult,
        scan: &ScanReport,
        threats: &ThreatReport,
        compliance: &ComplianceResult,
        policy: &PolicyEvaluationResult,
    ) -> SecurityScore {
        let mut score = 100u32; // Start with perfect score
        
        // Signature verification (30% weight)
        match signature {
            signing::VerificationResult::Valid { .. } => {}, // No penalty
            signing::VerificationResult::InvalidSignature => score = score.saturating_sub(30),
            signing::VerificationResult::UntrustedPublisher => score = score.saturating_sub(20),
            signing::VerificationResult::Unsigned => score = score.saturating_sub(15),
            signing::VerificationResult::RevokedKey => score = score.saturating_sub(35),
            signing::VerificationResult::ExpiredSignature => score = score.saturating_sub(25),
            signing::VerificationResult::ModifiedContent => score = score.saturating_sub(40),
            signing::VerificationResult::InvalidChainOfTrust => score = score.saturating_sub(20),
            signing::VerificationResult::MissingSignature => score = score.saturating_sub(10),
        }
        
        // Static analysis findings (25% weight)
        for finding in &scan.findings {
            let penalty = match finding.severity {
                Severity::Critical => 25,
                Severity::High => 15,
                Severity::Medium => 8,
                Severity::Low => 3,
                Severity::Info => 1,
            };
            score = score.saturating_sub(penalty);
        }
        
        // Threat detection (20% weight)
        if threats.malware_detected {
            score = 0; // Automatic failure
        } else {
            score = score.saturating_sub(threats.suspicion_score);
        }
        
        // Compliance violations (15% weight)
        for violation in &compliance.violations {
            let penalty = match violation.severity {
                Severity::Critical => 15,
                Severity::High => 10,
                Severity::Medium => 5,
                Severity::Low => 2,
                Severity::Info => 1,
            };
            score = score.saturating_sub(penalty);
        }
        
        // Policy violations (10% weight)
        if !policy.approved {
            score = score.saturating_sub(10);
        }
        
        SecurityScore {
            overall_score: score,
            signature_score: signature.trust_score(),
            static_analysis_score: 100 - (scan.findings.len() as u32 * 5).min(50),
            threat_score: 100 - threats.suspicion_score,
            compliance_score: 100 - (compliance.violations.len() as u32 * 10).min(50),
            policy_score: if policy.approved { 100 } else { 50 },
        }
    }
}

/// Comprehensive security evaluation result
#[derive(Debug, Clone, Serialize)]
pub struct SecurityEvaluation {
    pub id: uuid::Uuid,
    pub plugin_path: std::path::PathBuf,
    pub signature_verification: signing::VerificationResult,
    pub static_analysis: ScanReport,
    pub threat_analysis: ThreatReport,
    pub compliance_verification: ComplianceResult,
    pub policy_evaluation: PolicyEvaluationResult,
    pub security_score: SecurityScore,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Security scoring breakdown
#[derive(Debug, Clone, Serialize)]
pub struct SecurityScore {
    pub overall_score: u32,     // 0-100 overall security score
    pub signature_score: u32,   // Code signing verification score
    pub static_analysis_score: u32, // Static code analysis score
    pub threat_score: u32,      // Threat detection score
    pub compliance_score: u32,  // Compliance verification score
    pub policy_score: u32,      // Policy adherence score
}

/// Security configuration for the framework
#[derive(Debug, Clone)]
pub struct SecurityConfig {
    pub enable_static_analysis: bool,
    pub enable_malware_detection: bool,
    pub enable_sandboxing: bool,
    pub strict_compliance: bool,
    pub audit_all_events: bool,
    pub enterprise_policies: bool,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            enable_static_analysis: true,
            enable_malware_detection: true,
            enable_sandboxing: true,
            strict_compliance: false,
            audit_all_events: true,
            enterprise_policies: false,
        }
    }
}

/// Initialize the security framework with logging
pub fn init() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("aegis_security=info")
        .with_target(false)
        .try_init()
        .ok(); // Ignore error if already initialized
    
    info!("Aegis Security Framework initialized");
    Ok(())
}