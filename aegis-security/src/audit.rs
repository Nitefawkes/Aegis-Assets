//! Enterprise audit logging and security event tracking

use anyhow::{Result, Context};
use rusqlite::{Connection, params};
use rusqlite_migration::{Migrations, M};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn, error};

use crate::{SecurityEvaluation, SandboxedExecution, scanning::ScanReport};

/// Enterprise audit logger for security events
pub struct AuditLogger {
    db: Arc<Mutex<Connection>>,
    config: AuditConfig,
}

impl AuditLogger {
    /// Create a new audit logger
    pub async fn new() -> Result<Self> {
        let config = AuditConfig::default();
        let db_path = &config.database_path;
        
        // Ensure directory exists
        if let Some(parent) = db_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        
        // Open database connection
        let mut conn = Connection::open(db_path)?;
        
        // Run migrations
        let migrations = Migrations::new(vec![
            M::up(include_str!("../migrations/001_audit_tables.sql")),
            M::up(include_str!("../migrations/002_audit_indexes.sql")),
            M::up(include_str!("../migrations/003_audit_partitions.sql")),
        ]);
        
        migrations.to_latest(&mut conn)?;
        
        info!("Audit database initialized: {}", db_path.display());
        
        Ok(Self {
            db: Arc::new(Mutex::new(conn)),
            config,
        })
    }
    
    /// Log a security evaluation event
    pub async fn log_security_evaluation(&self, evaluation: &SecurityEvaluation) -> Result<()> {
        let event = SecurityEvent {
            id: uuid::Uuid::new_v4(),
            event_type: SecurityEventType::PluginSecurityEvaluation,
            severity: self.determine_severity_from_evaluation(evaluation),
            timestamp: chrono::Utc::now(),
            plugin_path: Some(evaluation.plugin_path.clone()),
            plugin_hash: Some(evaluation.static_analysis.findings.first()
                .map(|f| "plugin_hash".to_string()) // Would extract from evaluation
                .unwrap_or_default()),
            publisher_id: None, // Would extract from signature
            user_id: None,
            session_id: Some(evaluation.id.to_string()),
            source_ip: None,
            user_agent: None,
            details: serde_json::to_value(evaluation)?,
            risk_score: Some(evaluation.security_score.overall_score),
            compliance_status: None,
            remediation_required: evaluation.security_score.overall_score < 50,
            related_events: Vec::new(),
        };
        
        self.store_event(&event).await?;
        
        // Trigger alerts if necessary
        if event.severity >= Severity::High {
            self.send_security_alert(&event).await?;
        }
        
        Ok(())
    }
    
    /// Log plugin execution event
    pub async fn log_plugin_execution(&self, plugin_path: &Path, execution: &SandboxedExecution) -> Result<()> {
        let event = SecurityEvent {
            id: uuid::Uuid::new_v4(),
            event_type: SecurityEventType::PluginExecution,
            severity: if execution.violations.is_empty() { 
                Severity::Info 
            } else { 
                Severity::Medium 
            },
            timestamp: chrono::Utc::now(),
            plugin_path: Some(plugin_path.to_path_buf()),
            plugin_hash: None,
            publisher_id: None,
            user_id: None,
            session_id: None,
            source_ip: None,
            user_agent: None,
            details: serde_json::to_value(execution)?,
            risk_score: None,
            compliance_status: None,
            remediation_required: !execution.violations.is_empty(),
            related_events: Vec::new(),
        };
        
        self.store_event(&event).await?;
        Ok(())
    }
    
    /// Log compliance violation
    pub async fn log_compliance_violation(&self, violation: &ComplianceViolation) -> Result<()> {
        let event = SecurityEvent {
            id: uuid::Uuid::new_v4(),
            event_type: SecurityEventType::ComplianceViolation,
            severity: violation.severity,
            timestamp: chrono::Utc::now(),
            plugin_path: violation.plugin_path.clone(),
            plugin_hash: None,
            publisher_id: violation.publisher_id.clone(),
            user_id: violation.user_id.clone(),
            session_id: None,
            source_ip: None,
            user_agent: None,
            details: serde_json::to_value(violation)?,
            risk_score: None,
            compliance_status: Some("VIOLATION".to_string()),
            remediation_required: true,
            related_events: Vec::new(),
        };
        
        self.store_event(&event).await?;
        self.send_security_alert(&event).await?;
        Ok(())
    }
    
    /// Log administrative action
    pub async fn log_admin_action(&self, action: &AdminAction) -> Result<()> {
        let event = SecurityEvent {
            id: uuid::Uuid::new_v4(),
            event_type: SecurityEventType::AdminAction,
            severity: Severity::Info,
            timestamp: chrono::Utc::now(),
            plugin_path: action.plugin_path.clone(),
            plugin_hash: None,
            publisher_id: action.target_publisher_id.clone(),
            user_id: Some(action.admin_user_id.clone()),
            session_id: action.session_id.clone(),
            source_ip: action.source_ip.clone(),
            user_agent: action.user_agent.clone(),
            details: serde_json::to_value(action)?,
            risk_score: None,
            compliance_status: None,
            remediation_required: false,
            related_events: Vec::new(),
        };
        
        self.store_event(&event).await?;
        Ok(())
    }
    
    /// Store security event in database
    async fn store_event(&self, event: &SecurityEvent) -> Result<()> {
        let db = self.db.lock().await;
        
        db.execute(
            "INSERT INTO security_events (
                id, event_type, severity, timestamp, plugin_path, plugin_hash,
                publisher_id, user_id, session_id, source_ip, user_agent,
                details, risk_score, compliance_status, remediation_required
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15
            )",
            params![
                event.id.to_string(),
                event.event_type.to_string(),
                event.severity.to_string(),
                event.timestamp.timestamp(),
                event.plugin_path.as_ref().map(|p| p.to_string_lossy().to_string()),
                event.plugin_hash,
                event.publisher_id,
                event.user_id,
                event.session_id,
                event.source_ip,
                event.user_agent,
                serde_json::to_string(&event.details)?,
                event.risk_score,
                event.compliance_status,
                event.remediation_required
            ],
        )?;
        
        // Store related events
        for related_id in &event.related_events {
            db.execute(
                "INSERT INTO event_relationships (event_id, related_event_id) VALUES (?1, ?2)",
                params![event.id.to_string(), related_id.to_string()],
            )?;
        }
        
        Ok(())
    }
    
    /// Send security alert for high-severity events
    async fn send_security_alert(&self, event: &SecurityEvent) -> Result<()> {
        if !self.config.enable_alerts {
            return Ok(());
        }
        
        // Create alert
        let alert = SecurityAlert {
            id: uuid::Uuid::new_v4(),
            event_id: event.id,
            alert_type: self.determine_alert_type(event),
            severity: event.severity,
            title: self.generate_alert_title(event),
            description: self.generate_alert_description(event),
            created_at: chrono::Utc::now(),
            acknowledged: false,
            acknowledged_by: None,
            acknowledged_at: None,
            resolved: false,
            resolved_by: None,
            resolved_at: None,
        };
        
        // Store alert
        self.store_alert(&alert).await?;
        
        // Send notifications
        self.send_alert_notifications(&alert).await?;
        
        Ok(())
    }
    
    async fn store_alert(&self, alert: &SecurityAlert) -> Result<()> {
        let db = self.db.lock().await;
        
        db.execute(
            "INSERT INTO security_alerts (
                id, event_id, alert_type, severity, title, description,
                created_at, acknowledged, resolved
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                alert.id.to_string(),
                alert.event_id.to_string(),
                alert.alert_type.to_string(),
                alert.severity.to_string(),
                alert.title,
                alert.description,
                alert.created_at.timestamp(),
                alert.acknowledged,
                alert.resolved
            ],
        )?;
        
        Ok(())
    }
    
    async fn send_alert_notifications(&self, alert: &SecurityAlert) -> Result<()> {
        // Implementation would send notifications via:
        // - Email
        // - Slack/Teams
        // - SIEM systems
        // - PagerDuty/OpsGenie
        
        info!("Security alert: {} - {}", alert.alert_type, alert.title);
        Ok(())
    }
    
    /// Query security events with filters
    pub async fn query_events(&self, query: &AuditQuery) -> Result<Vec<SecurityEvent>> {
        let db = self.db.lock().await;
        
        let mut sql = "SELECT * FROM security_events WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        
        // Add filters
        if let Some(start_time) = query.start_time {
            sql.push_str(" AND timestamp >= ?");
            params.push(Box::new(start_time.timestamp()));
        }
        
        if let Some(end_time) = query.end_time {
            sql.push_str(" AND timestamp <= ?");
            params.push(Box::new(end_time.timestamp()));
        }
        
        if let Some(event_type) = &query.event_type {
            sql.push_str(" AND event_type = ?");
            params.push(Box::new(event_type.to_string()));
        }
        
        if let Some(severity) = query.min_severity {
            sql.push_str(" AND severity >= ?");
            params.push(Box::new(severity.to_string()));
        }
        
        if let Some(plugin_path) = &query.plugin_path {
            sql.push_str(" AND plugin_path = ?");
            params.push(Box::new(plugin_path.to_string_lossy().to_string()));
        }
        
        sql.push_str(" ORDER BY timestamp DESC");
        
        if let Some(limit) = query.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }
        
        let mut stmt = db.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        
        let event_iter = stmt.query_map(&param_refs[..], |row| {
            Ok(SecurityEvent {
                id: uuid::Uuid::parse_str(&row.get::<_, String>(0)?)
                    .map_err(|e| rusqlite::Error::InvalidColumnType(0, "id".to_string(), rusqlite::types::Type::Text))?,
                event_type: serde_json::from_str(&row.get::<_, String>(1)?)
                    .map_err(|e| rusqlite::Error::InvalidColumnType(1, "event_type".to_string(), rusqlite::types::Type::Text))?,
                severity: serde_json::from_str(&row.get::<_, String>(2)?)
                    .map_err(|e| rusqlite::Error::InvalidColumnType(2, "severity".to_string(), rusqlite::types::Type::Text))?,
                timestamp: chrono::DateTime::from_timestamp(row.get(3)?, 0).unwrap_or_default(),
                plugin_path: row.get::<_, Option<String>>(4)?.map(PathBuf::from),
                plugin_hash: row.get(5)?,
                publisher_id: row.get(6)?,
                user_id: row.get(7)?,
                session_id: row.get(8)?,
                source_ip: row.get(9)?,
                user_agent: row.get(10)?,
                details: serde_json::from_str(&row.get::<_, String>(11)?)
                    .map_err(|e| rusqlite::Error::InvalidColumnType(11, "details".to_string(), rusqlite::types::Type::Text))?,
                risk_score: row.get(12)?,
                compliance_status: row.get(13)?,
                remediation_required: row.get(14)?,
                related_events: Vec::new(), // Would be populated separately
            })
        })?;
        
        event_iter.collect::<Result<Vec<_>, _>>().map_err(|e| e.into())
    }
    
    /// Generate audit trail report
    pub async fn generate_audit_trail(&self, query: &AuditQuery) -> Result<AuditTrail> {
        let events = self.query_events(query).await?;
        
        let summary = self.generate_audit_summary(&events);
        
        let trail = AuditTrail {
            id: uuid::Uuid::new_v4(),
            generated_at: chrono::Utc::now(),
            query_parameters: query.clone(),
            total_events: events.len(),
            events,
            summary,
        };
        
        Ok(trail)
    }
    
    fn generate_audit_summary(&self, events: &[SecurityEvent]) -> AuditSummary {
        let mut summary = AuditSummary {
            total_events: events.len(),
            events_by_type: std::collections::HashMap::new(),
            events_by_severity: std::collections::HashMap::new(),
            high_risk_events: 0,
            compliance_violations: 0,
            remediation_required: 0,
        };
        
        for event in events {
            // Count by type
            *summary.events_by_type.entry(event.event_type.clone()).or_insert(0) += 1;
            
            // Count by severity
            *summary.events_by_severity.entry(event.severity).or_insert(0) += 1;
            
            // High risk events
            if event.risk_score.unwrap_or(0) > 70 {
                summary.high_risk_events += 1;
            }
            
            // Compliance violations
            if matches!(event.event_type, SecurityEventType::ComplianceViolation) {
                summary.compliance_violations += 1;
            }
            
            // Remediation required
            if event.remediation_required {
                summary.remediation_required += 1;
            }
        }
        
        summary
    }
    
    fn determine_severity_from_evaluation(&self, evaluation: &SecurityEvaluation) -> Severity {
        match evaluation.security_score.overall_score {
            0..=30 => Severity::Critical,
            31..=50 => Severity::High,
            51..=70 => Severity::Medium,
            71..=85 => Severity::Low,
            _ => Severity::Info,
        }
    }
    
    fn determine_alert_type(&self, event: &SecurityEvent) -> AlertType {
        match event.event_type {
            SecurityEventType::MalwareDetected => AlertType::SecurityThreat,
            SecurityEventType::ComplianceViolation => AlertType::ComplianceIssue,
            SecurityEventType::PrivilegeEscalation => AlertType::SecurityThreat,
            SecurityEventType::SuspiciousActivity => AlertType::SecurityThreat,
            _ => AlertType::Informational,
        }
    }
    
    fn generate_alert_title(&self, event: &SecurityEvent) -> String {
        match event.event_type {
            SecurityEventType::MalwareDetected => "Malware detected in plugin".to_string(),
            SecurityEventType::ComplianceViolation => "Compliance policy violation".to_string(),
            SecurityEventType::PluginSecurityEvaluation => "High-risk plugin detected".to_string(),
            _ => format!("Security event: {:?}", event.event_type),
        }
    }
    
    fn generate_alert_description(&self, event: &SecurityEvent) -> String {
        // Generate detailed description based on event details
        format!("Security event {} occurred at {}", event.event_type.to_string(), event.timestamp)
    }
}

// ===== DATA STRUCTURES =====

/// Security event record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityEvent {
    pub id: uuid::Uuid,
    pub event_type: SecurityEventType,
    pub severity: Severity,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub plugin_path: Option<PathBuf>,
    pub plugin_hash: Option<String>,
    pub publisher_id: Option<String>,
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub source_ip: Option<String>,
    pub user_agent: Option<String>,
    pub details: serde_json::Value,
    pub risk_score: Option<u32>,
    pub compliance_status: Option<String>,
    pub remediation_required: bool,
    pub related_events: Vec<uuid::Uuid>,
}

/// Types of security events
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum SecurityEventType {
    PluginSecurityEvaluation,
    PluginExecution,
    MalwareDetected,
    ComplianceViolation,
    PrivilegeEscalation,
    SuspiciousActivity,
    AdminAction,
    PolicyViolation,
    CertificateRevoked,
    UnauthorizedAccess,
}

impl std::fmt::Display for SecurityEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// Security alert
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityAlert {
    pub id: uuid::Uuid,
    pub event_id: uuid::Uuid,
    pub alert_type: AlertType,
    pub severity: Severity,
    pub title: String,
    pub description: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub acknowledged: bool,
    pub acknowledged_by: Option<String>,
    pub acknowledged_at: Option<chrono::DateTime<chrono::Utc>>,
    pub resolved: bool,
    pub resolved_by: Option<String>,
    pub resolved_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Alert types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertType {
    SecurityThreat,
    ComplianceIssue,
    PolicyViolation,
    SystemAnomaly,
    Informational,
}

impl std::fmt::Display for AlertType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// Severity levels
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// Compliance violation record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceViolation {
    pub violation_type: String,
    pub description: String,
    pub severity: Severity,
    pub plugin_path: Option<PathBuf>,
    pub publisher_id: Option<String>,
    pub user_id: Option<String>,
    pub policy_name: String,
    pub policy_version: String,
}

/// Administrative action record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminAction {
    pub action_type: String,
    pub description: String,
    pub admin_user_id: String,
    pub target_publisher_id: Option<String>,
    pub plugin_path: Option<PathBuf>,
    pub session_id: Option<String>,
    pub source_ip: Option<String>,
    pub user_agent: Option<String>,
}

/// Audit query parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditQuery {
    pub start_time: Option<chrono::DateTime<chrono::Utc>>,
    pub end_time: Option<chrono::DateTime<chrono::Utc>>,
    pub event_type: Option<SecurityEventType>,
    pub min_severity: Option<Severity>,
    pub plugin_path: Option<PathBuf>,
    pub publisher_id: Option<String>,
    pub user_id: Option<String>,
    pub limit: Option<usize>,
}

/// Audit trail report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditTrail {
    pub id: uuid::Uuid,
    pub generated_at: chrono::DateTime<chrono::Utc>,
    pub query_parameters: AuditQuery,
    pub total_events: usize,
    pub events: Vec<SecurityEvent>,
    pub summary: AuditSummary,
}

/// Audit summary statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditSummary {
    pub total_events: usize,
    pub events_by_type: std::collections::HashMap<SecurityEventType, usize>,
    pub events_by_severity: std::collections::HashMap<Severity, usize>,
    pub high_risk_events: usize,
    pub compliance_violations: usize,
    pub remediation_required: usize,
}

/// Audit configuration
#[derive(Debug, Clone)]
pub struct AuditConfig {
    pub database_path: PathBuf,
    pub enable_alerts: bool,
    pub retention_days: u32,
    pub alert_endpoints: Vec<String>,
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            database_path: PathBuf::from("./audit.db"),
            enable_alerts: true,
            retention_days: 365,
            alert_endpoints: Vec::new(),
        }
    }
}