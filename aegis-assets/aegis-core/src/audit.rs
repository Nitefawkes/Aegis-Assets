use crate::events::ExtractionEvent;
use anyhow::{Context, Result};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use uuid::Uuid;

pub struct AuditLogger {
    file: Mutex<File>,
    path: PathBuf,
}

impl AuditLogger {
    pub fn new(log_dir: &Path, job_id: Uuid) -> Result<Self> {
        fs::create_dir_all(log_dir).context("Failed to create audit log directory")?;
        let path = log_dir.join(format!("extraction-{}.jsonl", job_id));
        let file = File::create(&path).context("Failed to create audit log file")?;
        Ok(Self {
            file: Mutex::new(file),
            path,
        })
    }

    pub fn log_event(&self, event: &ExtractionEvent) -> Result<()> {
        let mut file = self
            .file
            .lock()
            .expect("audit log mutex poisoned");
        let payload = serde_json::to_string(event).context("Failed to serialize audit event")?;
        writeln!(file, "{}", payload).context("Failed to write audit log entry")?;
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}
