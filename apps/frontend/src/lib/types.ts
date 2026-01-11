// User types
export interface User {
  id: number;
  username: string;
  role: 'admin' | 'user';
  created_at: string;
}

// Media status types
export type MediaStatus = 'missing' | 'searching' | 'downloading' | 'processing' | 'available';
export type AlbumStatus = 'missing' | 'searching' | 'downloading' | 'processing' | 'partial' | 'available';
export type ShowStatus = 'continuing' | 'ended' | 'canceled' | 'upcoming';

// Movie types
export interface Movie {
  id: number;
  tmdb_id: number;
  imdb_id?: string;
  title: string;
  original_title?: string;
  year: number;
  overview?: string;
  poster_path?: string;
  backdrop_path?: string;
  runtime_minutes?: number;
  genres: string[];
  status: MediaStatus;
  monitored: boolean;
  quality_limit: string;
  file_path?: string;
  file_size?: number;
  added_at: string;
}

// TV types
export interface TvShow {
  id: number;
  tmdb_id: number;
  imdb_id?: string;
  title: string;
  original_title?: string;
  year_start?: number;
  year_end?: number;
  overview?: string;
  poster_path?: string;
  backdrop_path?: string;
  status: ShowStatus;
  monitored: boolean;
  quality_limit: string;
  seasons: Season[];
  added_at: string;
}

export interface Season {
  season_number: number;
  episode_count: number;
  available_count: number;
  episodes: Episode[];
}

export interface Episode {
  id: number;
  show_id: number;
  tmdb_id?: number;
  season_number: number;
  episode_number: number;
  title?: string;
  overview?: string;
  air_date?: string;
  runtime_minutes?: number;
  still_path?: string;
  status: MediaStatus;
  monitored: boolean;
  file_path?: string;
  file_size?: number;
}

// Music types
export interface Artist {
  id: number;
  mbid: string;
  name: string;
  sort_name?: string;
  disambiguation?: string;
  artist_type?: string;
  country?: string;
  begin_date?: string;
  end_date?: string;
  overview?: string;
  image_path?: string;
  monitored: boolean;
  quality_limit: string;
  albums: Album[];
  added_at: string;
}

export interface Album {
  id: number;
  mbid: string;
  artist_id: number;
  title: string;
  album_type?: string;
  release_date?: string;
  overview?: string;
  cover_path?: string;
  total_tracks?: number;
  status: AlbumStatus;
  monitored: boolean;
  quality_limit: string;
  tracks: Track[];
  added_at: string;
}

export interface Track {
  id: number;
  mbid?: string;
  album_id: number;
  artist_id?: number;
  title: string;
  track_number: number;
  disc_number: number;
  duration_ms?: number;
  status: MediaStatus;
  monitored: boolean;
  file_path?: string;
  file_size?: number;
  audio_format?: string;
  bitrate?: number;
  sample_rate?: number;
  bit_depth?: number;
}

// Download types
export type DownloadStatus = 'queued' | 'downloading' | 'seeding' | 'processing' | 'completed' | 'failed' | 'paused';

export interface Download {
  id: number;
  info_hash: string;
  name: string;
  media_type: 'movie' | 'episode' | 'album' | 'track';
  media_id: number;
  status: DownloadStatus;
  progress: number;
  download_speed: number;
  upload_speed: number;
  size_bytes?: number;
  downloaded_bytes: number;
  uploaded_bytes: number;
  ratio: number;
  peers: number;
  error_message?: string;
  added_at: string;
  started_at?: string;
  completed_at?: string;
}

// Release types
export type Quality = '2160p' | '1080p' | '720p' | '480p' | 'unknown';
export type Source = 'bluray' | 'webdl' | 'webrip' | 'hdtv' | 'dvd' | 'cam' | 'unknown';

export interface Release {
  id: string;
  title: string;
  indexer: string;
  magnet: string;
  size_bytes: number;
  seeders: number;
  leechers: number;
  quality: Quality;
  source: Source;
  codec?: string;
  audio?: string;
  group?: string;
  proper: boolean;
  repack: boolean;
  uploaded_at: string;
}

// System types
export interface SystemStatus {
  version: string;
  uptime_seconds: number;
  database_size_bytes: number;
  downloads: {
    active: number;
    queued: number;
    seeding: number;
  };
  storage: {
    mounts: MountStatus[];
  };
  vpn: {
    enabled: boolean;
    interface: string;
    connected: boolean;
    public_ip?: string;
  };
}

export interface MountStatus {
  name: string;
  type: 'local' | 'smb';
  available: boolean;
  total_bytes?: number;
  free_bytes?: number;
  error?: string;
}

// Pagination
export interface PaginatedResponse<T> {
  items: T[];
  total: number;
  page: number;
  pages: number;
}

// External search result types
export interface TmdbMovieSearchResult {
  id: number;
  title: string;
  original_title?: string;
  overview?: string;
  poster_path?: string;
  backdrop_path?: string;
  release_date?: string;
  vote_average?: number;
  vote_count?: number;
  genre_ids?: number[];
}

export interface TmdbTvSearchResult {
  id: number;
  name: string;
  original_name?: string;
  overview?: string;
  poster_path?: string;
  backdrop_path?: string;
  first_air_date?: string;
  vote_average?: number;
  vote_count?: number;
  genre_ids?: number[];
}

export interface MusicBrainzArtistSearchResult {
  id: string;
  name: string;
  sort_name?: string;
  disambiguation?: string;
  type?: string;
  country?: string;
  score?: number;
}

export interface MusicBrainzAlbumSearchResult {
  id: string;
  title: string;
  artist_credit?: string;
  release_date?: string;
  type?: string;
  country?: string;
  score?: number;
}

// Activity event types
export type ActivityEventType =
  | 'media:added'
  | 'media:updated'
  | 'media:deleted'
  | 'download:started'
  | 'download:completed'
  | 'download:failed'
  | 'system:startup'
  | 'system:error';

export interface ActivityEvent {
  id: number;
  type: ActivityEventType;
  message: string;
  details?: Record<string, unknown>;
  created_at: string;
}
