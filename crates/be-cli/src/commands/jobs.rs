use anyhow::Result;
use clap::Subcommand;
use colored::Colorize;

use be_core::store::JobStore;

#[derive(Subcommand)]
pub enum JobsCommands {
    /// List all jobs
    #[command(name = "")]
    List,
    /// Delete all job history
    Clear,
    /// Delete a specific job
    Delete {
        /// Job ID
        job_id: String,
    },
}

pub async fn run(cmd: JobsCommands) -> Result<()> {
    match cmd {
        JobsCommands::List => list_jobs(),
        JobsCommands::Clear => clear_jobs(),
        JobsCommands::Delete { job_id } => delete_job(&job_id),
    }
}

fn list_jobs() -> Result<()> {
    let store = JobStore::new()?;
    let jobs = store.list_jobs()?;

    if jobs.is_empty() {
        println!("No jobs found. Run `be run <bee> --input data.csv` to start one.");
        return Ok(());
    }

    println!(
        "{:<10} {:<20} {:<30} {:<10} {:<8} {:<8} {}",
        "JOB ID".bold(), "BEE".bold(), "MODEL".bold(), "STATUS".bold(),
        "DONE".bold(), "FAIL".bold(), "CREATED".bold()
    );
    println!("{}", "─".repeat(100));

    for job in &jobs {
        let status_colored = match job.status {
            be_core::store::JobStatus::Done => job.status.to_string().green().to_string(),
            be_core::store::JobStatus::Running => job.status.to_string().cyan().to_string(),
            be_core::store::JobStatus::DoneWithErrors => job.status.to_string().yellow().to_string(),
            be_core::store::JobStatus::Failed => job.status.to_string().red().to_string(),
        };

        println!(
            "{:<10} {:<20} {:<30} {:<10} {:<8} {:<8} {}",
            &job.id[..8],
            truncate(&job.bee_name, 18),
            truncate(&job.model, 28),
            status_colored,
            job.completed,
            job.failed,
            job.created_at.format("%Y-%m-%d %H:%M"),
        );
    }

    println!("\n{} job(s)", jobs.len());
    Ok(())
}

fn clear_jobs() -> Result<()> {
    use dialoguer::Confirm;

    let confirmed = Confirm::new()
        .with_prompt("Delete ALL job history? This cannot be undone.")
        .default(false)
        .interact()?;

    if !confirmed {
        println!("Aborted.");
        return Ok(());
    }

    let store = JobStore::new()?;
    let count = store.delete_all_jobs()?;
    println!("{} Deleted {} job(s).", "✓".green(), count);
    Ok(())
}

fn delete_job(job_id: &str) -> Result<()> {
    let store = JobStore::new()?;
    store.delete_job(job_id)?;
    println!("{} Job '{}' deleted.", "✓".green(), job_id);
    Ok(())
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max - 1])
    }
}
