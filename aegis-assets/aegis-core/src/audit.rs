use crate::events::ExtractionEvent;
use anyhow::{Context, Result};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use uuid::Uuid;

struct AuditHashState {
    index: u64,
    prev_hash: blake3::Hash,
}

pub struct AuditLogger {
    file: Mutex<File>,
    hash_file: Mutex<File>,
    hash_state: Mutex<AuditHashState>,
    path: PathBuf,
    hash_path: PathBuf,
}

impl AuditLogger {
    pub fn new(log_dir: &Path, job_id: Uuid) -> Result<Self> {
        fs::create_dir_all(log_dir).context("Failed to create audit log directory")?;
        let path = log_dir.join(format!("extraction-{}.jsonl", job_id));
        let file = File::create(&path).context("Failed to create audit log file")?;
        let hash_path = log_dir.join(format!("extraction-{}.jsonl.blake3", job_id));
        let hash_file = File::create(&hash_path).context("Failed to create audit hash file")?;
        Ok(Self {
            file: Mutex::new(file),
            hash_file: Mutex::new(hash_file),
            hash_state: Mutex::new(AuditHashState {
                index: 0,
                prev_hash: blake3::hash(&[]),
            }),
            path,
            hash_path,
        })
    }

    pub fn log_event(&self, event: &ExtractionEvent) -> Result<()> {
        let mut file = self
            .file
            .lock()
            .expect("audit log mutex poisoned");
        let mut hash_file = self
            .hash_file
            .lock()
            .expect("audit hash mutex poisoned");
        let mut hash_state = self
            .hash_state
            .lock()
            .expect("audit hash state mutex poisoned");
        let payload = serde_json::to_string(event).context("Failed to serialize audit event")?;
        writeln!(file, "{}", payload).context("Failed to write audit log entry")?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(hash_state.prev_hash.as_bytes());
        hasher.update(payload.as_bytes());
        let hash = hasher.finalize();
        writeln!(hash_file, "{} {}", hash_state.index, hash.to_hex())
            .context("Failed to write audit hash entry")?;
        hash_state.prev_hash = hash;
        hash_state.index += 1;
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
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
