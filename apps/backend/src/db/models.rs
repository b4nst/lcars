use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UserRole {
    Admin,
    User,
}

impl std::fmt::Display for UserRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UserRole::Admin => write!(f, "admin"),
            UserRole::User => write!(f, "user"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MediaStatus {
    Missing,
    Searching,
    Downloading,
    Processing,
    Available,
}

impl std::fmt::Display for MediaStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MediaStatus::Missing => write!(f, "missing"),
            MediaStatus::Searching => write!(f, "searching"),
            MediaStatus::Downloading => write!(f, "downloading"),
            MediaStatus::Processing => write!(f, "processing"),
            MediaStatus::Available => write!(f, "available"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AlbumStatus {
    Missing,
    Searching,
    Downloading,
    Processing,
    Partial,
    Available,
}

impl std::fmt::Display for AlbumStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AlbumStatus::Missing => write!(f, "missing"),
            AlbumStatus::Searching => write!(f, "searching"),
            AlbumStatus::Downloading => write!(f, "downloading"),
            AlbumStatus::Processing => write!(f, "processing"),
            AlbumStatus::Partial => write!(f, "partial"),
            AlbumStatus::Available => write!(f, "available"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShowStatus {
    Continuing,
    Ended,
    Canceled,
    Upcoming,
}

impl std::fmt::Display for ShowStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShowStatus::Continuing => write!(f, "continuing"),
            ShowStatus::Ended => write!(f, "ended"),
            ShowStatus::Canceled => write!(f, "canceled"),
            ShowStatus::Upcoming => write!(f, "upcoming"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DownloadStatus {
    Queued,
    Downloading,
    Seeding,
    Processing,
    Completed,
    Failed,
    Paused,
}

impl std::fmt::Display for DownloadStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DownloadStatus::Queued => write!(f, "queued"),
            DownloadStatus::Downloading => write!(f, "downloading"),
            DownloadStatus::Seeding => write!(f, "seeding"),
            DownloadStatus::Processing => write!(f, "processing"),
            DownloadStatus::Completed => write!(f, "completed"),
            DownloadStatus::Failed => write!(f, "failed"),
            DownloadStatus::Paused => write!(f, "paused"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MediaType {
    Movie,
    Episode,
    Album,
    Track,
}

impl std::fmt::Display for MediaType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MediaType::Movie => write!(f, "movie"),
            MediaType::Episode => write!(f, "episode"),
            MediaType::Album => write!(f, "album"),
            MediaType::Track => write!(f, "track"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub password_hash: String,
    pub role: UserRole,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Movie {
    pub id: i64,
    pub tmdb_id: i64,
    pub imdb_id: Option<String>,
    pub title: String,
    pub original_title: Option<String>,
    pub year: i32,
    pub overview: Option<String>,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
    pub runtime_minutes: Option<i32>,
    pub genres: Option<String>,
    pub status: MediaStatus,
    pub monitored: bool,
    pub quality_limit: String,
    pub file_path: Option<String>,
    pub file_size: Option<i64>,
    pub added_at: String,
    pub updated_at: String,
    pub added_by: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TvShow {
    pub id: i64,
    pub tmdb_id: i64,
    pub imdb_id: Option<String>,
    pub title: String,
    pub original_title: Option<String>,
    pub year_start: Option<i32>,
    pub year_end: Option<i32>,
    pub overview: Option<String>,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
    pub status: ShowStatus,
    pub monitored: bool,
    pub quality_limit: String,
    pub added_at: String,
    pub updated_at: String,
    pub added_by: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Episode {
    pub id: i64,
    pub show_id: i64,
    pub tmdb_id: Option<i64>,
    pub season_number: i32,
    pub episode_number: i32,
    pub title: Option<String>,
    pub overview: Option<String>,
    pub air_date: Option<String>,
    pub runtime_minutes: Option<i32>,
    pub still_path: Option<String>,
    pub status: MediaStatus,
    pub monitored: bool,
    pub file_path: Option<String>,
    pub file_size: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artist {
    pub id: i64,
    pub mbid: String,
    pub name: String,
    pub sort_name: Option<String>,
    pub disambiguation: Option<String>,
    pub artist_type: Option<String>,
    pub country: Option<String>,
    pub begin_date: Option<String>,
    pub end_date: Option<String>,
    pub overview: Option<String>,
    pub image_path: Option<String>,
    pub monitored: bool,
    pub quality_limit: String,
    pub added_at: String,
    pub updated_at: String,
    pub added_by: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Album {
    pub id: i64,
    pub mbid: String,
    pub artist_id: i64,
    pub title: String,
    pub album_type: Option<String>,
    pub release_date: Option<String>,
    pub overview: Option<String>,
    pub cover_path: Option<String>,
    pub total_tracks: Option<i32>,
    pub status: AlbumStatus,
    pub monitored: bool,
    pub quality_limit: String,
    pub added_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    pub id: i64,
    pub mbid: Option<String>,
    pub album_id: i64,
    pub artist_id: Option<i64>,
    pub title: String,
    pub track_number: i32,
    pub disc_number: i32,
    pub duration_ms: Option<i32>,
    pub status: MediaStatus,
    pub monitored: bool,
    pub file_path: Option<String>,
    pub file_size: Option<i64>,
    pub audio_format: Option<String>,
    pub bitrate: Option<i32>,
    pub sample_rate: Option<i32>,
    pub bit_depth: Option<i32>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Indexer {
    pub id: i64,
    pub name: String,
    pub indexer_type: String,
    pub url: String,
    pub api_key: Option<String>,
    pub enabled: bool,
    pub priority: i32,
    pub categories: Option<String>,
    pub last_check: Option<String>,
    pub last_error: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Download {
    pub id: i64,
    pub info_hash: String,
    pub name: String,
    pub media_type: MediaType,
    pub media_id: i64,
    pub magnet: String,
    pub status: DownloadStatus,
    pub progress: f64,
    pub download_speed: i64,
    pub upload_speed: i64,
    pub size_bytes: Option<i64>,
    pub downloaded_bytes: i64,
    pub uploaded_bytes: i64,
    pub ratio: f64,
    pub peers: i32,
    pub error_message: Option<String>,
    pub added_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Activity {
    pub id: i64,
    pub event_type: String,
    pub message: String,
    pub media_type: Option<String>,
    pub media_id: Option<i64>,
    pub download_id: Option<i64>,
    pub user_id: Option<i64>,
    pub metadata: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: i64,
    pub user_id: i64,
    pub token_hash: String,
    pub expires_at: String,
    pub created_at: String,
}
