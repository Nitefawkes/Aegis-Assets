use crate::Config;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{create_dir_all, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use thiserror::Error;
use uuid::Uuid;

pub const AUDIT_LOG_FILENAME: &str = "audit-log.jsonl";

#[derive(Debug, Error)]
pub enum AuditError {
    #[error("Audit logging is disabled")]
    Disabled,
    #[error("Failed to write audit event: {0}")]
    WriteFailed(#[from] std::io::Error),
    #[error("Failed to serialize audit event: {0}")]
    SerializeFailed(#[from] serde_json::Error),
    #[error("Audit log path unavailable")]
    MissingPath,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub job_id: Uuid,
    pub event: AuditEventKind,
}

impl AuditEvent {
    pub fn new(job_id: Uuid, event: AuditEventKind) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            job_id,
            event,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type", rename_all = "snake_case")]
pub enum AuditEventKind {
    JobStarted {
        source_path: String,
        output_dir: String,
    },
    JobCompleted {
        success: bool,
        duration_ms: u64,
        resource_count: usize,
        error_message: Option<String>,
    },
    ComplianceDecision {
        is_compliant: bool,
        risk_level: String,
        warnings: Vec<String>,
        recommendations: Vec<String>,
        verification_required: bool,
    },
    PluginUsed {
        plugin_name: String,
        plugin_version: String,
    },
    OutputGenerated {
        output_path: String,
        resource_name: String,
        resource_type: String,
        estimated_memory_bytes: usize,
    },
}

#[derive(Debug, Clone)]
pub struct AuditLogWriter {
    enabled: bool,
    log_path: Option<PathBuf>,
}

impl AuditLogWriter {
    pub fn from_config(config: &Config) -> Self {
        let log_path = config
            .enterprise_config
            .as_ref()
            .map(|enterprise| enterprise.audit_log_dir.join(AUDIT_LOG_FILENAME));
        let enabled = config
            .enterprise_config
            .as_ref()
            .map(|enterprise| enterprise.enable_audit_logs)
            .unwrap_or(false);
        Self { enabled, log_path }
    }

    pub fn log_path(&self) -> Option<&Path> {
        self.log_path.as_deref()
    }

    pub fn log_event(&self, event: &AuditEvent) -> Result<(), AuditError> {
        if !self.enabled {
            return Err(AuditError::Disabled);
        }
        let log_path = self.log_path.as_ref().ok_or(AuditError::MissingPath)?;
        if let Some(parent) = log_path.parent() {
            create_dir_all(parent)?;
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)?;
        serde_json::to_writer(&mut file, event)?;
        writeln!(file)?;
        file.sync_data()?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct AuditLogReader {
    log_path: PathBuf,
}

impl AuditLogReader {
    pub fn from_config(config: &Config) -> Option<Self> {
        let log_path = config
            .enterprise_config
            .as_ref()
            .map(|enterprise| enterprise.audit_log_dir.join(AUDIT_LOG_FILENAME))?;
        Some(Self { log_path })
    }

    pub fn read_events(&self, limit: Option<usize>) -> Result<Vec<AuditEvent>, AuditError> {
        let file = std::fs::File::open(&self.log_path)?;
        let reader = BufReader::new(file);
        let mut events = Vec::new();

        for line in reader.lines() {
            let line = line?;
            let event: AuditEvent = serde_json::from_str(&line)?;
            events.push(event);
            if let Some(max) = limit {
                if events.len() >= max {
                    break;
                }
            }
        }

        Ok(events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Config, EnterpriseConfig};
    use tempfile::TempDir;

    #[test]
    fn test_audit_log_writer_and_reader() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let config = Config {
            enterprise_config: Some(EnterpriseConfig {
                enable_audit_logs: true,
                audit_log_dir: temp_dir.path().to_path_buf(),
                require_compliance_verification: false,
                steam_api_key: None,
                epic_api_key: None,
            }),
            ..Config::default()
        };

        let writer = AuditLogWriter::from_config(&config);
        let job_id = Uuid::new_v4();
        let event = AuditEvent::new(
            job_id,
            AuditEventKind::JobStarted {
                source_path: "/tmp/source".to_string(),
                output_dir: "/tmp/out".to_string(),
            },
        );

        writer.log_event(&event).expect("Failed to write event");

        let reader = AuditLogReader::from_config(&config).expect("Missing reader");
        let events = reader.read_events(Some(10)).expect("Failed to read events");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].job_id, job_id);
    }

    pub fn hash_path(&self) -> &Path {
        &self.hash_path
    }
}

pub fn verify_audit_log(log_path: &Path, hash_path: &Path) -> Result<()> {
    let log_file = File::open(log_path).context("Failed to open audit log file")?;
    let hash_file = File::open(hash_path).context("Failed to open audit hash file")?;
    let log_reader = BufReader::new(log_file);
    let hash_reader = BufReader::new(hash_file);
    let mut prev_hash = blake3::hash(&[]);

    for (index, (log_line, hash_line)) in log_reader
        .lines()
        .zip(hash_reader.lines())
        .enumerate()
    {
        let log_line = log_line.context("Failed to read audit log line")?;
        let hash_line = hash_line.context("Failed to read audit hash line")?;
        let mut parts = hash_line.split_whitespace();
        let reported_index = parts
            .next()
            .context("Missing audit hash index")?
            .parse::<u64>()
            .context("Invalid audit hash index")?;
        let reported_hash = parts
            .next()
            .context("Missing audit hash value")?;
        if reported_index != index as u64 {
            anyhow::bail!(
                "Audit hash index mismatch: expected {}, got {}",
                index,
                reported_index
            );
        }

        let mut hasher = blake3::Hasher::new();
        hasher.update(prev_hash.as_bytes());
        hasher.update(log_line.as_bytes());
        let computed = hasher.finalize();
        if computed.to_hex().as_str() != reported_hash {
            anyhow::bail!(
                "Audit hash mismatch at index {}: expected {}, got {}",
                index,
                computed.to_hex(),
                reported_hash
            );
        }

        prev_hash = computed;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{ExtractionEventKind, JobState};
    use chrono::Utc;
    use tempfile::TempDir;

    #[test]
    fn test_audit_log_hash_chain() -> Result<()> {
        let tmp = TempDir::new().context("create temp dir")?;
        let job_id = Uuid::new_v4();
        let logger = AuditLogger::new(tmp.path(), job_id)?;
        let event = ExtractionEvent {
            job_id,
            occurred_at: Utc::now(),
            kind: ExtractionEventKind::JobStateChange {
                state: JobState::Queued,
                message: Some("Test event".to_string()),
            },
        };
        logger.log_event(&event)?;
        verify_audit_log(logger.path(), logger.hash_path())?;
        Ok(())
    }
}
