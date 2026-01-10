//! Naming pattern engine for generating media file paths.
//!
//! Supports customizable patterns with placeholders for media metadata.

use lazy_static::lazy_static;
use regex::Regex;

use crate::config::NamingConfig;

use super::MediaInfo;

lazy_static! {
    /// Regex to match placeholders in naming patterns.
    /// Matches: {name} or {name:02} (with format specifier)
    static ref PLACEHOLDER_REGEX: Regex = Regex::new(r"\{(\w+)(?::(\d+))?\}").unwrap();

    /// Characters that are unsafe in filenames across platforms.
    static ref UNSAFE_CHARS: Regex = Regex::new(r#"[<>:"/\\|?*\x00-\x1F]"#).unwrap();
}

/// Engine for generating file paths from naming patterns.
///
/// Supports the following placeholders:
///
/// ## Movies/TV:
/// - `{title}` - Media title (sanitized for filesystem)
/// - `{year}` - Release year
/// - `{quality}` - e.g., "1080p"
/// - `{season:02}` - Zero-padded season number
/// - `{episode:02}` - Zero-padded episode number
/// - `{episode_title}` - Episode title
/// - `{ext}` - File extension (without dot)
///
/// ## Music:
/// - `{artist}` - Artist name
/// - `{album}` - Album title
/// - `{title}` - Track title
/// - `{track:02}` - Zero-padded track number
/// - `{disc:02}` - Zero-padded disc number
/// - `{ext}` - File extension
pub struct NamingEngine {
    movie_pattern: String,
    tv_pattern: String,
    music_pattern: String,
}

impl NamingEngine {
    /// Creates a new naming engine from configuration.
    pub fn new(config: NamingConfig) -> Self {
        Self {
            movie_pattern: config.movie_pattern,
            tv_pattern: config.tv_pattern,
            music_pattern: config.music_pattern,
        }
    }

    /// Generates a file path for the given media info and file extension.
    pub fn generate_path(&self, media_info: &MediaInfo, ext: &str) -> String {
        match media_info {
            MediaInfo::Movie { movie, quality } => {
                self.generate_movie_path(&movie.title, movie.year, quality, ext)
            }
            MediaInfo::Episode {
                show,
                episode,
                quality,
            } => self.generate_episode_path(
                &show.title,
                episode.season_number,
                episode.episode_number,
                episode.title.as_deref().unwrap_or(""),
                quality,
                ext,
            ),
            MediaInfo::Album { artist, album } => {
                // For album, we can't generate individual track paths
                // This is typically used for album art or similar
                self.generate_album_path(&artist.name, &album.title, ext)
            }
            MediaInfo::Track {
                artist,
                album,
                track,
            } => self.generate_track_path(
                &artist.name,
                &album.title,
                &track.title,
                track.track_number,
                track.disc_number,
                ext,
            ),
        }
    }

    /// Generates a movie file path from the configured pattern.
    pub fn generate_movie_path(&self, title: &str, year: i32, quality: &str, ext: &str) -> String {
        let mut result = self.movie_pattern.clone();

        result = replace_placeholder(&result, "title", &sanitize_filename(title));
        result = replace_placeholder(&result, "year", &year.to_string());
        result = replace_placeholder(&result, "quality", &sanitize_filename(quality));
        result = replace_placeholder(&result, "ext", ext);

        result
    }

    /// Generates a TV episode file path from the configured pattern.
    pub fn generate_episode_path(
        &self,
        show_title: &str,
        season: i32,
        episode: i32,
        episode_title: &str,
        quality: &str,
        ext: &str,
    ) -> String {
        let mut result = self.tv_pattern.clone();

        result = replace_placeholder(&result, "title", &sanitize_filename(show_title));
        result = replace_placeholder_padded(&result, "season", season);
        result = replace_placeholder_padded(&result, "episode", episode);
        result = replace_placeholder(&result, "episode_title", &sanitize_filename(episode_title));
        result = replace_placeholder(&result, "quality", &sanitize_filename(quality));
        result = replace_placeholder(&result, "ext", ext);

        result
    }

    /// Generates a music track file path from the configured pattern.
    pub fn generate_track_path(
        &self,
        artist: &str,
        album: &str,
        title: &str,
        track_num: i32,
        disc_num: i32,
        ext: &str,
    ) -> String {
        let mut result = self.music_pattern.clone();

        result = replace_placeholder(&result, "artist", &sanitize_filename(artist));
        result = replace_placeholder(&result, "album", &sanitize_filename(album));
        result = replace_placeholder(&result, "title", &sanitize_filename(title));
        result = replace_placeholder_padded(&result, "track", track_num);
        result = replace_placeholder_padded(&result, "disc", disc_num);
        result = replace_placeholder(&result, "ext", ext);

        result
    }

    /// Generates an album directory path (for album art, etc.).
    fn generate_album_path(&self, artist: &str, album: &str, ext: &str) -> String {
        // Use the music pattern but strip the track-specific parts
        // Generate path like: music/{artist}/{album}/cover.{ext}
        format!(
            "music/{}/{}/cover.{}",
            sanitize_filename(artist),
            sanitize_filename(album),
            ext
        )
    }

    /// Returns the movie pattern.
    pub fn movie_pattern(&self) -> &str {
        &self.movie_pattern
    }

    /// Returns the TV pattern.
    pub fn tv_pattern(&self) -> &str {
        &self.tv_pattern
    }

    /// Returns the music pattern.
    pub fn music_pattern(&self) -> &str {
        &self.music_pattern
    }
}

/// Replaces a placeholder with a value.
///
/// Handles both `{name}` and `{name:XX}` formats.
fn replace_placeholder(pattern: &str, name: &str, value: &str) -> String {
    let simple_placeholder = format!("{{{}}}", name);
    let mut result = pattern.replace(&simple_placeholder, value);

    // Also handle padded versions like {name:02} with the raw value
    let padded_pattern = format!(r"\{{{name}:(\d+)\}}");
    if let Ok(re) = Regex::new(&padded_pattern) {
        result = re.replace_all(&result, value).to_string();
    }

    result
}

/// Replaces a placeholder with a zero-padded number.
///
/// If the pattern contains `{name:02}`, pads to 2 digits.
/// Falls back to no padding if no format specifier is found.
fn replace_placeholder_padded(pattern: &str, name: &str, value: i32) -> String {
    let padded_pattern = format!(r"\{{{name}:(\d+)\}}");

    if let Ok(re) = Regex::new(&padded_pattern) {
        if let Some(caps) = re.captures(pattern) {
            if let Some(width_match) = caps.get(1) {
                if let Ok(width) = width_match.as_str().parse::<usize>() {
                    let padded_value = format!("{:0width$}", value, width = width);
                    return re.replace_all(pattern, padded_value.as_str()).to_string();
                }
            }
        }
    }

    // Fall back to simple replacement
    replace_placeholder(pattern, name, &value.to_string())
}

/// Sanitizes a string for use in filenames.
///
/// - Replaces unsafe characters with underscores
/// - Trims leading/trailing whitespace and dots
/// - Collapses multiple spaces/underscores
/// - Truncates to reasonable length
pub fn sanitize_filename(name: &str) -> String {
    let mut result = UNSAFE_CHARS.replace_all(name, "_").to_string();

    // Collapse multiple underscores or spaces
    while result.contains("__") {
        result = result.replace("__", "_");
    }
    while result.contains("  ") {
        result = result.replace("  ", " ");
    }

    // Trim leading/trailing whitespace, underscores, and dots
    result = result
        .trim_matches(|c| c == ' ' || c == '_' || c == '.')
        .to_string();

    // Truncate to 200 characters (leaving room for extensions)
    if result.len() > 200 {
        result = result[..200].to_string();
        // Clean up any partial multi-byte characters
        while !result.is_empty() && !result.is_char_boundary(result.len()) {
            result.pop();
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::NamingConfig;

    fn test_config() -> NamingConfig {
        NamingConfig {
            movie_pattern: "movies/{title} ({year})/{title} ({year}) - {quality}.{ext}".to_string(),
            tv_pattern: "tv/{title}/S{season:02}/{title} - S{season:02}E{episode:02} - {episode_title}.{ext}".to_string(),
            music_pattern: "music/{artist}/{album}/{track:02} - {title}.{ext}".to_string(),
        }
    }

    #[test]
    fn test_generate_movie_path() {
        let engine = NamingEngine::new(test_config());

        let path = engine.generate_movie_path("The Matrix", 1999, "1080p", "mkv");
        assert_eq!(
            path,
            "movies/The Matrix (1999)/The Matrix (1999) - 1080p.mkv"
        );
    }

    #[test]
    fn test_generate_movie_path_special_chars() {
        let engine = NamingEngine::new(test_config());

        let path = engine.generate_movie_path("Mission: Impossible", 1996, "720p", "mp4");
        assert_eq!(
            path,
            "movies/Mission_ Impossible (1996)/Mission_ Impossible (1996) - 720p.mp4"
        );
    }

    #[test]
    fn test_generate_episode_path() {
        let engine = NamingEngine::new(test_config());

        let path = engine.generate_episode_path("Breaking Bad", 5, 16, "Felina", "1080p", "mkv");
        assert_eq!(
            path,
            "tv/Breaking Bad/S05/Breaking Bad - S05E16 - Felina.mkv"
        );
    }

    #[test]
    fn test_generate_episode_path_single_digit() {
        let engine = NamingEngine::new(test_config());

        let path = engine.generate_episode_path("Friends", 1, 1, "Pilot", "720p", "mp4");
        assert_eq!(path, "tv/Friends/S01/Friends - S01E01 - Pilot.mp4");
    }

    #[test]
    fn test_generate_track_path() {
        let engine = NamingEngine::new(test_config());

        let path = engine.generate_track_path(
            "Pink Floyd",
            "The Dark Side of the Moon",
            "Time",
            4,
            1,
            "flac",
        );
        assert_eq!(
            path,
            "music/Pink Floyd/The Dark Side of the Moon/04 - Time.flac"
        );
    }

    #[test]
    fn test_generate_track_path_special_chars() {
        let engine = NamingEngine::new(test_config());

        let path = engine.generate_track_path(
            "AC/DC",
            "Back in Black",
            "You Shook Me All Night Long",
            6,
            1,
            "mp3",
        );
        assert_eq!(
            path,
            "music/AC_DC/Back in Black/06 - You Shook Me All Night Long.mp3"
        );
    }

    #[test]
    fn test_sanitize_filename_basic() {
        assert_eq!(sanitize_filename("Normal Name"), "Normal Name");
    }

    #[test]
    fn test_sanitize_filename_special_chars() {
        assert_eq!(sanitize_filename("File: Name?"), "File_ Name");
        assert_eq!(sanitize_filename("Path/To\\File"), "Path_To_File");
        assert_eq!(sanitize_filename("<>:\"|?*"), "");
    }

    #[test]
    fn test_sanitize_filename_trim() {
        assert_eq!(sanitize_filename("  Name  "), "Name");
        assert_eq!(sanitize_filename("..Name.."), "Name");
        assert_eq!(sanitize_filename("__Name__"), "Name");
    }

    #[test]
    fn test_sanitize_filename_collapse_underscores() {
        assert_eq!(
            sanitize_filename("Name___With___Underscores"),
            "Name_With_Underscores"
        );
    }

    #[test]
    fn test_sanitize_filename_unicode() {
        assert_eq!(sanitize_filename("Café Résumé"), "Café Résumé");
        assert_eq!(sanitize_filename("日本語"), "日本語");
    }

    #[test]
    fn test_replace_placeholder_padded() {
        assert_eq!(
            replace_placeholder_padded("S{season:02}E{episode:02}", "season", 1),
            "S01E{episode:02}"
        );
        assert_eq!(
            replace_placeholder_padded("Track {track:03}", "track", 5),
            "Track 005"
        );
    }

    #[test]
    fn test_custom_pattern() {
        let config = NamingConfig {
            movie_pattern: "{title}/{title}.{ext}".to_string(),
            tv_pattern: "{title}/Season {season:02}/{episode:02}.{ext}".to_string(),
            music_pattern: "{artist} - {album} - {title}.{ext}".to_string(),
        };

        let engine = NamingEngine::new(config);

        assert_eq!(
            engine.generate_movie_path("Test", 2020, "1080p", "mkv"),
            "Test/Test.mkv"
        );

        assert_eq!(
            engine.generate_episode_path("Show", 2, 5, "Episode", "720p", "mp4"),
            "Show/Season 02/05.mp4"
        );

        assert_eq!(
            engine.generate_track_path("Artist", "Album", "Song", 1, 1, "flac"),
            "Artist - Album - Song.flac"
        );
    }
}
