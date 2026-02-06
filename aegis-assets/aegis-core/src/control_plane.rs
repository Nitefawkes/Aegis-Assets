use crate::audit::{AuditError, AuditEvent, AuditLogReader};
use crate::Config;
use std::collections::VecDeque;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum ControlPlaneError {
    #[error("Audit log access unavailable")]
    AuditUnavailable,
    #[error(transparent)]
    AuditError(#[from] AuditError),
}

#[derive(Debug, Clone)]
pub struct ControlPlane {
    audit_reader: Option<AuditLogReader>,
}

impl ControlPlane {
    pub fn new(config: &Config) -> Self {
        let audit_reader = AuditLogReader::from_config(config);
        Self { audit_reader }
    }

    pub fn list_audit_events(
        &self,
        limit: Option<usize>,
    ) -> Result<Vec<AuditEvent>, ControlPlaneError> {
        let reader = self
            .audit_reader
            .as_ref()
            .ok_or(ControlPlaneError::AuditUnavailable)?;
        Ok(reader.read_events(limit)?)
    }

    pub fn list_audit_events_for_job(
        &self,
        job_id: Uuid,
        limit: Option<usize>,
    ) -> Result<Vec<AuditEvent>, ControlPlaneError> {
        let reader = self
            .audit_reader
            .as_ref()
            .ok_or(ControlPlaneError::AuditUnavailable)?;
        let events = reader.read_events(None)?;
        let mut filtered = VecDeque::new();

        for event in events.into_iter().filter(|event| event.job_id == job_id) {
            filtered.push_back(event);
            if let Some(max) = limit {
                if filtered.len() >= max {
                    break;
                }
            }
        }

        Ok(filtered.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::{AuditEventKind, AuditLogWriter};
    use crate::EnterpriseConfig;
    use tempfile::TempDir;

    #[test]
    fn test_control_plane_reads_events() {
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
            AuditEventKind::PluginUsed {
                plugin_name: "mock".to_string(),
                plugin_version: "0.0.1".to_string(),
            },
        );
        writer.log_event(&event).expect("Failed to write event");

        let control_plane = ControlPlane::new(&config);
        let events = control_plane
            .list_audit_events_for_job(job_id, Some(5))
            .expect("Failed to list events");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].job_id, job_id);
    }
}
