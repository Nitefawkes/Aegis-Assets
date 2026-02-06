use crate::archive::ComplianceLevel;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionEvent {
    pub job_id: Uuid,
    pub occurred_at: DateTime<Utc>,
    pub kind: ExtractionEventKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExtractionEventKind {
    JobStateChange {
        state: JobState,
        message: Option<String>,
    },
    ComplianceDecision {
        is_compliant: bool,
        risk_level: ComplianceLevel,
        warnings: Vec<String>,
        recommendations: Vec<String>,
    },
    AssetIndexingProgress {
        indexed: usize,
        total: usize,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Queued,
    Running,
    Completed,
    Failed,
}

pub trait ExtractionEventEmitter: Send + Sync {
    fn emit(&self, event: ExtractionEvent);
}

#[derive(Debug, Default)]
pub struct NoopEventEmitter;

impl ExtractionEventEmitter for NoopEventEmitter {
    fn emit(&self, _event: ExtractionEvent) {}
}
