//! Scheduler service for running background jobs on a schedule.
//!
//! Manages scheduled tasks like searching for missing media, refreshing metadata,
//! checking for new episodes/releases, and cleaning up completed downloads.

use std::sync::Arc;

use rusqlite::Connection;
use tokio::sync::Mutex;
use tokio_cron_scheduler::{Job, JobScheduler, JobSchedulerError};

use crate::config::SchedulerConfig;
use crate::error::{AppError, Result};
use crate::services::indexer::{MediaSearchType, SearchQuery};
use crate::services::{IndexerManager, MusicBrainzClient, TmdbClient, TorrentEngine};

/// Job execution context providing access to application services.
#[derive(Clone)]
pub struct JobContext {
    pub db: Arc<Mutex<Connection>>,
    pub tmdb_client: Option<Arc<TmdbClient>>,
    pub musicbrainz_client: Option<Arc<MusicBrainzClient>>,
    pub indexer_manager: Arc<IndexerManager>,
    pub torrent_engine: Option<Arc<TorrentEngine>>,
}

/// The scheduler service managing all background jobs.
pub struct Scheduler {
    scheduler: JobScheduler,
}

impl Scheduler {
    /// Create a new scheduler wrapped in Arc for shared access.
    pub async fn new_shared(config: &SchedulerConfig, ctx: JobContext) -> Result<Arc<Self>> {
        Ok(Arc::new(Self::new(config, ctx).await?))
    }

    /// Create a new scheduler with all configured jobs.
    pub async fn new(config: &SchedulerConfig, ctx: JobContext) -> Result<Self> {
        let scheduler = JobScheduler::new()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to create scheduler: {}", e)))?;

        // Add all scheduled jobs
        Self::add_search_missing_job(&scheduler, &config.search_missing, ctx.clone()).await?;
        Self::add_refresh_metadata_job(&scheduler, &config.refresh_metadata, ctx.clone()).await?;
        Self::add_check_new_episodes_job(&scheduler, &config.check_new_episodes, ctx.clone())
            .await?;
        Self::add_check_new_releases_job(&scheduler, &config.check_new_releases, ctx.clone())
            .await?;
        Self::add_cleanup_completed_job(&scheduler, &config.cleanup_completed, ctx).await?;

        Ok(Self { scheduler })
    }

    /// Start the scheduler.
    pub async fn start(&self) -> Result<()> {
        self.scheduler
            .start()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to start scheduler: {}", e)))
    }

    /// Shutdown the scheduler gracefully.
    pub async fn shutdown(mut self) -> Result<()> {
        self.scheduler
            .shutdown()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to shutdown scheduler: {}", e)))
    }

    /// Add the search missing media job.
    async fn add_search_missing_job(
        scheduler: &JobScheduler,
        cron: &str,
        ctx: JobContext,
    ) -> Result<()> {
        let job = Job::new_async(cron, move |_uuid, _lock| {
            let ctx = ctx.clone();
            Box::pin(async move {
                run_search_missing_job(&ctx).await;
            })
        })
        .map_err(map_scheduler_error)?;

        scheduler.add(job).await.map_err(map_scheduler_error)?;
        tracing::debug!(cron = cron, "Scheduled search_missing job");
        Ok(())
    }

    /// Add the refresh metadata job.
    async fn add_refresh_metadata_job(
        scheduler: &JobScheduler,
        cron: &str,
        ctx: JobContext,
    ) -> Result<()> {
        let job = Job::new_async(cron, move |_uuid, _lock| {
            let ctx = ctx.clone();
            Box::pin(async move {
                run_refresh_metadata_job(&ctx).await;
            })
        })
        .map_err(map_scheduler_error)?;

        scheduler.add(job).await.map_err(map_scheduler_error)?;
        tracing::debug!(cron = cron, "Scheduled refresh_metadata job");
        Ok(())
    }

    /// Add the check new episodes job.
    async fn add_check_new_episodes_job(
        scheduler: &JobScheduler,
        cron: &str,
        ctx: JobContext,
    ) -> Result<()> {
        let job = Job::new_async(cron, move |_uuid, _lock| {
            let ctx = ctx.clone();
            Box::pin(async move {
                run_check_new_episodes_job(&ctx).await;
            })
        })
        .map_err(map_scheduler_error)?;

        scheduler.add(job).await.map_err(map_scheduler_error)?;
        tracing::debug!(cron = cron, "Scheduled check_new_episodes job");
        Ok(())
    }

    /// Add the check new releases job (music).
    async fn add_check_new_releases_job(
        scheduler: &JobScheduler,
        cron: &str,
        ctx: JobContext,
    ) -> Result<()> {
        let job = Job::new_async(cron, move |_uuid, _lock| {
            let ctx = ctx.clone();
            Box::pin(async move {
                run_check_new_releases_job(&ctx).await;
            })
        })
        .map_err(map_scheduler_error)?;

        scheduler.add(job).await.map_err(map_scheduler_error)?;
        tracing::debug!(cron = cron, "Scheduled check_new_releases job");
        Ok(())
    }

    /// Add the cleanup completed downloads job.
    async fn add_cleanup_completed_job(
        scheduler: &JobScheduler,
        cron: &str,
        ctx: JobContext,
    ) -> Result<()> {
        let job = Job::new_async(cron, move |_uuid, _lock| {
            let ctx = ctx.clone();
            Box::pin(async move {
                run_cleanup_completed_job(&ctx).await;
            })
        })
        .map_err(map_scheduler_error)?;

        scheduler.add(job).await.map_err(map_scheduler_error)?;
        tracing::debug!(cron = cron, "Scheduled cleanup_completed job");
        Ok(())
    }
}

/// Map JobSchedulerError to AppError.
fn map_scheduler_error(e: JobSchedulerError) -> AppError {
    AppError::Internal(format!("Scheduler error: {}", e))
}

// ============================================================================
// Job Implementations
// ============================================================================

/// Search for missing media and queue downloads.
pub async fn run_search_missing_job(ctx: &JobContext) {
    tracing::info!("Running search_missing job");

    // Find monitored movies with status 'missing'
    if let Err(e) = search_missing_movies(ctx).await {
        tracing::error!(error = %e, "Failed to search missing movies");
    }

    // Find monitored episodes with status 'missing'
    if let Err(e) = search_missing_episodes(ctx).await {
        tracing::error!(error = %e, "Failed to search missing episodes");
    }

    // Find monitored albums with status 'missing'
    if let Err(e) = search_missing_albums(ctx).await {
        tracing::error!(error = %e, "Failed to search missing albums");
    }

    tracing::info!("search_missing job completed");
}

async fn search_missing_movies(ctx: &JobContext) -> Result<()> {
    let movies: Vec<(i64, String, Option<i32>)> = {
        let db = ctx.db.lock().await;
        let mut stmt = db.prepare(
            "SELECT id, title, year FROM movies WHERE status = 'missing' AND monitored = 1",
        )?;
        let result = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
            .filter_map(|r| r.ok())
            .collect();
        result
    };

    for (id, title, year) in movies {
        let mut query = SearchQuery::new(&title).media_type(MediaSearchType::Movie);

        if let Some(y) = year {
            query = query.year(y);
        }

        tracing::debug!(movie_id = id, query = %title, "Searching for missing movie");

        // Search indexers for this movie
        match ctx.indexer_manager.search(&query).await {
            Ok(results) if !results.is_empty() => {
                tracing::info!(
                    movie_id = id,
                    title = %title,
                    results = results.len(),
                    "Found releases for missing movie"
                );
                // TODO: Implement automatic selection and download queueing
            }
            Ok(_) => {
                tracing::debug!(movie_id = id, title = %title, "No releases found");
            }
            Err(e) => {
                tracing::warn!(movie_id = id, error = %e, "Search failed for movie");
            }
        }
    }

    Ok(())
}

async fn search_missing_episodes(ctx: &JobContext) -> Result<()> {
    let episodes: Vec<(i64, String, i32, i32)> = {
        let db = ctx.db.lock().await;
        let mut stmt = db.prepare(
            r#"
            SELECT e.id, s.title, e.season_number, e.episode_number
            FROM episodes e
            JOIN shows s ON e.show_id = s.id
            WHERE e.status = 'missing' AND s.monitored = 1
            ORDER BY s.id, e.season_number, e.episode_number
            "#,
        )?;
        let result = stmt
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?
            .filter_map(|r| r.ok())
            .collect();
        result
    };

    for (id, show_title, season, episode) in episodes {
        let query = SearchQuery::new(&show_title)
            .media_type(MediaSearchType::TvEpisode)
            .episode(season, episode);

        tracing::debug!(episode_id = id, show = %show_title, season, episode, "Searching for missing episode");

        match ctx.indexer_manager.search(&query).await {
            Ok(results) if !results.is_empty() => {
                tracing::info!(
                    episode_id = id,
                    show = %show_title,
                    season = season,
                    episode = episode,
                    results = results.len(),
                    "Found releases for missing episode"
                );
                // TODO: Implement automatic selection and download queueing
            }
            Ok(_) => {
                tracing::debug!(episode_id = id, "No releases found");
            }
            Err(e) => {
                tracing::warn!(episode_id = id, error = %e, "Search failed for episode");
            }
        }
    }

    Ok(())
}

async fn search_missing_albums(ctx: &JobContext) -> Result<()> {
    let albums: Vec<(i64, String, String)> = {
        let db = ctx.db.lock().await;
        let mut stmt = db.prepare(
            r#"
            SELECT al.id, ar.name, al.title
            FROM albums al
            JOIN artists ar ON al.artist_id = ar.id
            WHERE al.status = 'missing' AND ar.monitored = 1
            "#,
        )?;
        let result = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
            .filter_map(|r| r.ok())
            .collect();
        result
    };

    for (id, artist, album_title) in albums {
        // Combine artist and album into search query
        let query = SearchQuery::new(format!("{} {}", artist, album_title))
            .media_type(MediaSearchType::MusicAlbum);

        tracing::debug!(album_id = id, artist = %artist, album = %album_title, "Searching for missing album");

        match ctx.indexer_manager.search(&query).await {
            Ok(results) if !results.is_empty() => {
                tracing::info!(
                    album_id = id,
                    artist = %artist,
                    album = %album_title,
                    results = results.len(),
                    "Found releases for missing album"
                );
                // TODO: Implement automatic selection and download queueing
            }
            Ok(_) => {
                tracing::debug!(album_id = id, "No releases found");
            }
            Err(e) => {
                tracing::warn!(album_id = id, error = %e, "Search failed for album");
            }
        }
    }

    Ok(())
}

/// Refresh metadata from external sources.
pub async fn run_refresh_metadata_job(ctx: &JobContext) {
    tracing::info!("Running refresh_metadata job");

    // Refresh movie metadata from TMDB
    if let Some(tmdb) = &ctx.tmdb_client {
        if let Err(e) = refresh_movie_metadata(ctx, tmdb).await {
            tracing::error!(error = %e, "Failed to refresh movie metadata");
        }
    }

    // Refresh TV show metadata (check for new seasons)
    if let Some(tmdb) = &ctx.tmdb_client {
        if let Err(e) = refresh_show_metadata(ctx, tmdb).await {
            tracing::error!(error = %e, "Failed to refresh show metadata");
        }
    }

    // Refresh artist/album metadata from MusicBrainz
    if let Some(mb) = &ctx.musicbrainz_client {
        if let Err(e) = refresh_music_metadata(ctx, mb).await {
            tracing::error!(error = %e, "Failed to refresh music metadata");
        }
    }

    tracing::info!("refresh_metadata job completed");
}

async fn refresh_movie_metadata(ctx: &JobContext, _tmdb: &TmdbClient) -> Result<()> {
    let movies: Vec<(i64, i64)> = {
        let db = ctx.db.lock().await;
        let mut stmt = db.prepare(
            "SELECT id, tmdb_id FROM movies WHERE tmdb_id IS NOT NULL ORDER BY updated_at ASC LIMIT 50",
        )?;
        let result = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .filter_map(|r| r.ok())
            .collect();
        result
    };

    for (id, _tmdb_id) in movies {
        tracing::debug!(movie_id = id, "Refreshing movie metadata");
        // TODO: Call TMDB API to refresh metadata
        // Update database with new metadata
    }

    Ok(())
}

async fn refresh_show_metadata(ctx: &JobContext, _tmdb: &TmdbClient) -> Result<()> {
    let shows: Vec<(i64, i64)> = {
        let db = ctx.db.lock().await;
        let mut stmt = db.prepare(
            "SELECT id, tmdb_id FROM shows WHERE tmdb_id IS NOT NULL ORDER BY updated_at ASC LIMIT 50",
        )?;
        let result = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .filter_map(|r| r.ok())
            .collect();
        result
    };

    for (id, _tmdb_id) in shows {
        tracing::debug!(show_id = id, "Refreshing show metadata");
        // TODO: Call TMDB API to refresh metadata
        // Check for new seasons/episodes
    }

    Ok(())
}

async fn refresh_music_metadata(ctx: &JobContext, _mb: &MusicBrainzClient) -> Result<()> {
    let artists: Vec<(i64, String)> = {
        let db = ctx.db.lock().await;
        let mut stmt = db.prepare(
            "SELECT id, musicbrainz_id FROM artists WHERE musicbrainz_id IS NOT NULL ORDER BY updated_at ASC LIMIT 50",
        )?;
        let result = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .filter_map(|r| r.ok())
            .collect();
        result
    };

    for (id, _mb_id) in artists {
        tracing::debug!(artist_id = id, "Refreshing artist metadata");
        // TODO: Call MusicBrainz API to refresh metadata
    }

    Ok(())
}

/// Check for new episodes of continuing TV shows.
pub async fn run_check_new_episodes_job(ctx: &JobContext) {
    tracing::info!("Running check_new_episodes job");

    let Some(tmdb) = &ctx.tmdb_client else {
        tracing::warn!("TMDB client not available, skipping new episode check");
        return;
    };

    if let Err(e) = check_new_episodes(ctx, tmdb).await {
        tracing::error!(error = %e, "Failed to check for new episodes");
    }

    tracing::info!("check_new_episodes job completed");
}

async fn check_new_episodes(ctx: &JobContext, _tmdb: &TmdbClient) -> Result<()> {
    let shows: Vec<(i64, String, i64)> = {
        let db = ctx.db.lock().await;
        let mut stmt = db.prepare(
            "SELECT id, title, tmdb_id FROM shows WHERE status = 'continuing' AND monitored = 1 AND tmdb_id IS NOT NULL",
        )?;
        let result = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
            .filter_map(|r| r.ok())
            .collect();
        result
    };

    for (id, title, _tmdb_id) in shows {
        tracing::debug!(show_id = id, title = %title, "Checking for new episodes");
        // TODO: Query TMDB for show details
        // Compare episodes with database
        // Add new episodes with 'missing' status
    }

    Ok(())
}

/// Check for new album releases from monitored artists.
pub async fn run_check_new_releases_job(ctx: &JobContext) {
    tracing::info!("Running check_new_releases job");

    let Some(mb) = &ctx.musicbrainz_client else {
        tracing::warn!("MusicBrainz client not available, skipping new release check");
        return;
    };

    if let Err(e) = check_new_releases(ctx, mb).await {
        tracing::error!(error = %e, "Failed to check for new releases");
    }

    tracing::info!("check_new_releases job completed");
}

async fn check_new_releases(ctx: &JobContext, _mb: &MusicBrainzClient) -> Result<()> {
    let artists: Vec<(i64, String, String)> = {
        let db = ctx.db.lock().await;
        let mut stmt = db.prepare(
            "SELECT id, name, musicbrainz_id FROM artists WHERE monitored = 1 AND musicbrainz_id IS NOT NULL",
        )?;
        let result = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
            .filter_map(|r| r.ok())
            .collect();
        result
    };

    for (id, name, _mb_id) in artists {
        tracing::debug!(artist_id = id, name = %name, "Checking for new releases");
        // TODO: Query MusicBrainz for artist release groups
        // Compare with database
        // Add new albums with 'missing' status
    }

    Ok(())
}

/// Clean up completed downloads that meet seeding requirements.
pub async fn run_cleanup_completed_job(ctx: &JobContext) {
    tracing::info!("Running cleanup_completed job");

    let Some(torrent_engine) = &ctx.torrent_engine else {
        tracing::debug!("Torrent engine not available, skipping cleanup");
        return;
    };

    if let Err(e) = cleanup_completed_downloads(ctx, torrent_engine).await {
        tracing::error!(error = %e, "Failed to cleanup completed downloads");
    }

    tracing::info!("cleanup_completed job completed");
}

async fn cleanup_completed_downloads(ctx: &JobContext, engine: &TorrentEngine) -> Result<()> {
    // Use the engine's built-in seeding completion check
    let completed = engine.check_seeding_completion().await;

    for info_hash in completed {
        tracing::info!(info_hash = %info_hash, "Removing completed torrent");

        // Remove torrent from engine (don't delete files - they've been processed)
        if let Err(e) = engine.remove(&info_hash, false).await {
            tracing::error!(
                info_hash = %info_hash,
                error = %e,
                "Failed to remove torrent"
            );
            continue;
        }

        // Update download status in database
        let db = ctx.db.lock().await;
        if let Err(e) = db.execute(
            "UPDATE downloads SET status = 'completed', completed_at = datetime('now') WHERE info_hash = ?",
            [&info_hash],
        ) {
            tracing::error!(
                info_hash = %info_hash,
                error = %e,
                "Failed to update download status"
            );
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_context_clone() {
        // JobContext must be Clone for use in async jobs
        fn assert_clone<T: Clone>() {}
        assert_clone::<JobContext>();
    }
}
