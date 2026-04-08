use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

use crate::bee::{Bee, BeeInput};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ResultStatus {
    Success,
    Failed,
}

impl std::fmt::Display for ResultStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResultStatus::Success => write!(f, "success"),
            ResultStatus::Failed => write!(f, "failed"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeeResult {
    pub index: usize,
    pub status: ResultStatus,
    pub input: BeeInput,
    pub output: Option<serde_json::Value>,
    pub error: Option<String>,
    #[serde(default)]
    pub retries_attempted: u32,
    pub tokens_input: u32,
    pub tokens_output: u32,
    pub cost_usd: f64,
    pub duration_ms: u64,
    pub model_used: String,
    pub completed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Running,
    Done,
    DoneWithErrors,
    Failed,
}

impl std::fmt::Display for JobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JobStatus::Running => write!(f, "running"),
            JobStatus::Done => write!(f, "done"),
            JobStatus::DoneWithErrors => write!(f, "done_with_errors"),
            JobStatus::Failed => write!(f, "failed"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobManifest {
    pub id: String,
    pub bee_name: String,
    pub model: String,
    pub total: usize,
    pub parallel: usize,
    pub status: JobStatus,
    pub created_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub completed: usize,
    pub failed: usize,
    pub total_cost_usd: f64,
    pub avg_duration_ms: u64,
}

pub struct JobStats {
    pub completed: usize,
    pub failed: usize,
    pub total_cost_usd: f64,
    pub avg_duration_ms: u64,
}

pub struct JobStore {
    base_dir: PathBuf,
}

impl JobStore {
    pub fn new() -> Result<Self> {
        let base_dir = dirs::home_dir()
            .ok_or_else(|| anyhow!("Cannot determine home directory"))?
            .join(".be")
            .join("jobs");
        std::fs::create_dir_all(&base_dir)?;
        Ok(Self { base_dir })
    }

    fn job_dir(&self, job_id: &str) -> PathBuf {
        self.base_dir.join(job_id)
    }

    fn manifest_path(&self, job_id: &str) -> PathBuf {
        self.job_dir(job_id).join("manifest.json")
    }

    fn result_path(&self, job_id: &str, index: usize) -> PathBuf {
        self.job_dir(job_id)
            .join("results")
            .join(format!("{:06}.json", index))
    }

    /// Create a job directory and manifest, return job_id
    pub async fn create_job(
        &self,
        bee: &Bee,
        total: usize,
        parallel: usize,
    ) -> Result<String> {
        let job_id = Uuid::new_v4().to_string();
        let job_dir = self.job_dir(&job_id);
        let results_dir = job_dir.join("results");

        std::fs::create_dir_all(&results_dir)?;

        let manifest = JobManifest {
            id: job_id.clone(),
            bee_name: bee.name.clone(),
            model: bee.model.clone(),
            total,
            parallel,
            status: JobStatus::Running,
            created_at: Utc::now(),
            finished_at: None,
            completed: 0,
            failed: 0,
            total_cost_usd: 0.0,
            avg_duration_ms: 0,
        };

        let json = serde_json::to_string_pretty(&manifest)?;
        tokio::fs::write(self.manifest_path(&job_id), json).await?;

        Ok(job_id)
    }

    /// Write one bee result file (called concurrently from many tokio tasks)
    pub async fn write_result(
        &self,
        job_id: &str,
        result: Result<BeeResult>,
    ) -> Result<()> {
        let bee_result = match result {
            Ok(r) => r,
            Err(e) => {
                // We need an index to write this, but we don't have one easily here
                // The caller should pass a proper BeeResult::Failed
                return Err(e);
            }
        };

        let path = self.result_path(job_id, bee_result.index);
        let json = serde_json::to_string_pretty(&bee_result)?;
        tokio::fs::write(path, json).await?;
        Ok(())
    }

    /// Write a failed bee result
    pub async fn write_failed_result(
        &self,
        job_id: &str,
        index: usize,
        input: BeeInput,
        error: String,
        retries: u32,
        duration_ms: u64,
    ) -> Result<()> {
        let result = BeeResult {
            index,
            status: ResultStatus::Failed,
            input,
            output: None,
            error: Some(error),
            retries_attempted: retries,
            tokens_input: 0,
            tokens_output: 0,
            cost_usd: 0.0,
            duration_ms,
            model_used: String::new(),
            completed_at: Utc::now(),
        };
        let path = self.result_path(job_id, index);
        let json = serde_json::to_string_pretty(&result)?;
        tokio::fs::write(path, json).await?;
        Ok(())
    }

    /// Update manifest with final stats
    pub async fn finish_job(
        &self,
        job_id: &str,
        stats: JobStats,
    ) -> Result<()> {
        let mut manifest = self.load_manifest(job_id)?;
        manifest.completed = stats.completed;
        manifest.failed = stats.failed;
        manifest.total_cost_usd = stats.total_cost_usd;
        manifest.avg_duration_ms = stats.avg_duration_ms;
        manifest.finished_at = Some(Utc::now());
        manifest.status = if stats.failed == 0 {
            JobStatus::Done
        } else if stats.completed == 0 {
            JobStatus::Failed
        } else {
            JobStatus::DoneWithErrors
        };

        let json = serde_json::to_string_pretty(&manifest)?;
        tokio::fs::write(self.manifest_path(job_id), json).await?;
        Ok(())
    }

    /// Load all results for a job, sorted by index
    pub fn load_results(&self, job_id: &str) -> Result<Vec<BeeResult>> {
        let results_dir = self.job_dir(job_id).join("results");
        if !results_dir.exists() {
            return Ok(Vec::new());
        }

        let mut results = Vec::new();
        let mut entries: Vec<_> = std::fs::read_dir(&results_dir)?
            .flatten()
            .collect();
        entries.sort_by_key(|e| e.file_name());

        for entry in entries {
            let path = entry.path();
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                let content = std::fs::read_to_string(&path)?;
                let result: BeeResult = serde_json::from_str(&content)
                    .map_err(|e| anyhow!("Failed to parse result {:?}: {}", path, e))?;
                results.push(result);
            }
        }

        Ok(results)
    }

    /// Load job manifest
    pub fn load_manifest(&self, job_id: &str) -> Result<JobManifest> {
        let path = self.manifest_path(job_id);
        let content = std::fs::read_to_string(&path)
            .map_err(|e| anyhow!("Job '{}' not found: {}", job_id, e))?;
        let manifest: JobManifest = serde_json::from_str(&content)?;
        Ok(manifest)
    }

    /// List all jobs, newest first
    pub fn list_jobs(&self) -> Result<Vec<JobManifest>> {
        if !self.base_dir.exists() {
            return Ok(Vec::new());
        }

        let mut manifests = Vec::new();
        for entry in std::fs::read_dir(&self.base_dir)?.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let manifest_path = path.join("manifest.json");
                if manifest_path.exists() {
                    if let Ok(content) = std::fs::read_to_string(&manifest_path) {
                        if let Ok(m) = serde_json::from_str::<JobManifest>(&content) {
                            manifests.push(m);
                        }
                    }
                }
            }
        }

        // Sort newest first
        manifests.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(manifests)
    }

    /// Delete a job and all its results
    pub fn delete_job(&self, job_id: &str) -> Result<()> {
        let job_dir = self.job_dir(job_id);
        if !job_dir.exists() {
            return Err(anyhow!("Job '{}' not found", job_id));
        }
        std::fs::remove_dir_all(job_dir)?;
        Ok(())
    }

    /// Delete all jobs
    pub fn delete_all_jobs(&self) -> Result<usize> {
        if !self.base_dir.exists() {
            return Ok(0);
        }
        let mut count = 0;
        for entry in std::fs::read_dir(&self.base_dir)?.flatten() {
            let path = entry.path();
            if path.is_dir() {
                std::fs::remove_dir_all(path)?;
                count += 1;
            }
        }
        Ok(count)
    }
}
