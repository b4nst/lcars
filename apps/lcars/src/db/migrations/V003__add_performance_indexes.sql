-- Performance optimization indexes for common query patterns
-- Created as part of Issue #20: End-to-end testing and polish

-- Movies: Compound index for filtered queries (status + monitored are common filters)
CREATE INDEX IF NOT EXISTS idx_movies_status_monitored ON movies(status, monitored);

-- Episodes: Compound index for show + season queries (very common pattern)
CREATE INDEX IF NOT EXISTS idx_episodes_show_season ON episodes(show_id, season_number);

-- Episodes: Index for status-based queries
CREATE INDEX IF NOT EXISTS idx_episodes_status ON episodes(status);

-- Albums: Compound index for artist + status queries
CREATE INDEX IF NOT EXISTS idx_albums_artist_status ON albums(artist_id, status);

-- Tracks: Index for album lookups
CREATE INDEX IF NOT EXISTS idx_tracks_album ON tracks(album_id);

-- Downloads: Index for sorting by added_at (most common sort order)
CREATE INDEX IF NOT EXISTS idx_downloads_added_at ON downloads(added_at DESC);

-- Downloads: Index for status-based queries
CREATE INDEX IF NOT EXISTS idx_downloads_status ON downloads(status);

-- Activity: Compound index for time-based + type queries
CREATE INDEX IF NOT EXISTS idx_activity_created_type ON activity(created_at DESC, event_type);

-- TV Shows: Index for status-based queries
CREATE INDEX IF NOT EXISTS idx_tv_shows_status ON tv_shows(status);

-- Artists: Index for monitored filter
CREATE INDEX IF NOT EXISTS idx_artists_monitored ON artists(monitored);
