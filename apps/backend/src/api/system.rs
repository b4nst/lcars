//! System API endpoints for job management and system status.

use axum::{
    extract::{Path, State},
    response::Json,
};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, Result};
use crate::services::scheduler::{
    run_check_new_episodes_job, run_check_new_releases_job, run_cleanup_completed_job,
    run_refresh_metadata_job, run_search_missing_job,
};
use crate::AppState;

/// Response for successful job trigger.
#[derive(Serialize)]
pub struct JobTriggerResponse {
    pub success: bool,
    pub job: String,
    pub message: String,
}

/// Available job names.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobName {
    SearchMissing,
    RefreshMetadata,
    CheckNewEpisodes,
    CheckNewReleases,
    CleanupCompleted,
}

impl std::fmt::Display for JobName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JobName::SearchMissing => write!(f, "search_missing"),
            JobName::RefreshMetadata => write!(f, "refresh_metadata"),
            JobName::CheckNewEpisodes => write!(f, "check_new_episodes"),
            JobName::CheckNewReleases => write!(f, "check_new_releases"),
            JobName::CleanupCompleted => write!(f, "cleanup_completed"),
        }
    }
}

/// Manually trigger a background job.
///
/// POST /api/system/jobs/:name/run
pub async fn trigger_job(
    State(state): State<AppState>,
    Path(job_name): Path<String>,
) -> Result<Json<JobTriggerResponse>> {
    let ctx = state.job_context();

    let job = match job_name.as_str() {
        "search_missing" => {
            tokio::spawn(async move {
                run_search_missing_job(&ctx).await;
            });
            JobName::SearchMissing
        }
        "refresh_metadata" => {
            tokio::spawn(async move {
                run_refresh_metadata_job(&ctx).await;
            });
            JobName::RefreshMetadata
        }
        "check_new_episodes" => {
            tokio::spawn(async move {
                run_check_new_episodes_job(&ctx).await;
            });
            JobName::CheckNewEpisodes
        }
        "check_new_releases" => {
            tokio::spawn(async move {
                run_check_new_releases_job(&ctx).await;
            });
            JobName::CheckNewReleases
        }
        "cleanup_completed" => {
            tokio::spawn(async move {
                run_cleanup_completed_job(&ctx).await;
            });
            JobName::CleanupCompleted
        }
        _ => {
            return Err(AppError::NotFound(format!("Job '{}' not found", job_name)));
        }
    };

    tracing::info!(job = %job, "Manually triggered job");

    Ok(Json(JobTriggerResponse {
        success: true,
        job: job.to_string(),
        message: format!("Job '{}' has been triggered", job),
    }))
}

/// List all available jobs.
///
/// GET /api/system/jobs
pub async fn list_jobs() -> Json<Vec<JobInfo>> {
    Json(vec![
        JobInfo {
            name: "search_missing".to_string(),
            description: "Search indexers for missing media and queue downloads".to_string(),
        },
        JobInfo {
            name: "refresh_metadata".to_string(),
            description: "Refresh metadata from TMDB and MusicBrainz".to_string(),
        },
        JobInfo {
            name: "check_new_episodes".to_string(),
            description: "Check for new episodes of continuing TV shows".to_string(),
        },
        JobInfo {
            name: "check_new_releases".to_string(),
            description: "Check for new album releases from monitored artists".to_string(),
        },
        JobInfo {
            name: "cleanup_completed".to_string(),
            description: "Clean up torrents that have met seeding requirements".to_string(),
        },
    ])
}

/// Job information for listing.
#[derive(Serialize)]
pub struct JobInfo {
    pub name: String,
    pub description: String,
}
