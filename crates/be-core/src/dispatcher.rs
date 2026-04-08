use anyhow::Result;
use std::sync::Arc;
use tokio::sync::{mpsc, Semaphore};

use crate::bee::{Bee, BeeInput};
use crate::runner;
use crate::store::{JobStats, JobStore, ResultStatus};

#[derive(Debug, Clone)]
pub enum BeeEvent {
    Started   { job_id: String, index: usize },
    Completed { job_id: String, index: usize, duration_ms: u64 },
    Failed    { job_id: String, index: usize, error: String },
    JobDone   { job_id: String, completed: usize, failed: usize },
}

pub struct DispatchConfig {
    pub parallel: usize,
    pub progress_tx: mpsc::Sender<BeeEvent>,
}

/// Dispatch a swarm of bees. Returns the job_id immediately.
/// Results are written to the store as each bee completes.
pub async fn dispatch(
    bee: Bee,
    inputs: Vec<BeeInput>,
    config: DispatchConfig,
    store: Arc<JobStore>,
) -> Result<String> {
    let total = inputs.len();
    let parallel = config.parallel.min(bee.max_parallel).max(1);

    let job_id = store.create_job(&bee, total, parallel).await?;
    let semaphore = Arc::new(Semaphore::new(parallel));

    let mut handles = Vec::with_capacity(total);

    for (index, input) in inputs.into_iter().enumerate() {
        let permit   = Arc::clone(&semaphore).acquire_owned().await?;
        let bee      = bee.clone();
        let store    = Arc::clone(&store);
        let tx       = config.progress_tx.clone();
        let job_id_c = job_id.clone();

        let handle = tokio::spawn(async move {
            let _permit = permit;

            tx.send(BeeEvent::Started {
                job_id: job_id_c.clone(),
                index,
            })
            .await
            .ok();

            let result = runner::run_bee(&bee, input, index).await;

            match &result {
                Ok(r) => {
                    tx.send(BeeEvent::Completed {
                        job_id: job_id_c.clone(),
                        index,
                        duration_ms: r.duration_ms,
                    })
                    .await
                    .ok();
                }
                Err(e) => {
                    tx.send(BeeEvent::Failed {
                        job_id: job_id_c.clone(),
                        index,
                        error: e.to_string(),
                    })
                    .await
                    .ok();
                }
            }

            // Write result — runner::run_bee always returns Ok (failures are encoded in BeeResult)
            if let Ok(bee_result) = result {
                store.write_result(&job_id_c, Ok(bee_result)).await.ok();
            }
        });

        handles.push(handle);
    }

    // Spawn a task to wait for all bees and finalize the job
    let store_clone = Arc::clone(&store);
    let job_id_clone = job_id.clone();
    let tx = config.progress_tx.clone();

    tokio::spawn(async move {
        for handle in handles {
            handle.await.ok();
        }

        // Calculate final stats from written results
        let stats = compute_stats(&store_clone, &job_id_clone);
        let (completed, failed, total_cost, avg_duration) = stats;

        store_clone
            .finish_job(
                &job_id_clone,
                JobStats {
                    completed,
                    failed,
                    total_cost_usd: total_cost,
                    avg_duration_ms: avg_duration,
                },
            )
            .await
            .ok();

        tx.send(BeeEvent::JobDone {
            job_id: job_id_clone,
            completed,
            failed,
        })
        .await
        .ok();
    });

    Ok(job_id)
}

fn compute_stats(store: &JobStore, job_id: &str) -> (usize, usize, f64, u64) {
    let results = match store.load_results(job_id) {
        Ok(r) => r,
        Err(_) => return (0, 0, 0.0, 0),
    };

    let mut completed = 0usize;
    let mut failed = 0usize;
    let mut total_cost = 0.0f64;
    let mut total_duration = 0u64;

    for r in &results {
        match r.status {
            ResultStatus::Success => {
                completed += 1;
                total_cost += r.cost_usd;
                total_duration += r.duration_ms;
            }
            ResultStatus::Failed => {
                failed += 1;
            }
        }
    }

    let avg_duration = if completed > 0 {
        total_duration / completed as u64
    } else {
        0
    };

    (completed, failed, total_cost, avg_duration)
}
