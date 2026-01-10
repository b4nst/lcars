//! Release name parser for extracting quality information from torrent release names.
//!
//! Parses video and music release names to extract quality indicators,
//! source information, codecs, and other metadata.

use lazy_static::lazy_static;
use regex::Regex;
use serde::Serialize;

lazy_static! {
    // Video quality patterns
    static ref QUALITY_RE: Regex = Regex::new(r"(?i)(2160p|1080p|720p|480p)").unwrap();
    static ref SOURCE_RE: Regex = Regex::new(r"(?i)(BluRay|Blu-Ray|BDRip|BRRip|WEB-DL|WEBDL|WEBRip|WEB|HDTV|DVDRip|DVD|CAM|TS|TELESYNC|HDCAM|SCR|SCREENER)").unwrap();
    static ref SEASON_EP_RE: Regex = Regex::new(r"(?i)S(\d{1,2})E(\d{1,2})").unwrap();
    static ref YEAR_RE: Regex = Regex::new(r"[.\s\(\[](\d{4})[.\s\)\]]").unwrap();
    static ref CODEC_RE: Regex = Regex::new(r"(?i)(x264|x265|HEVC|H\.?264|H\.?265|AVC|XviD|DivX)").unwrap();
    static ref AUDIO_RE: Regex = Regex::new(r"(?i)(AAC|AC3|DD5\.?1|DTS|DTS-HD|Atmos|TrueHD|FLAC|MP3|EAC3|E-AC-3)").unwrap();
    static ref GROUP_RE: Regex = Regex::new(r"-([A-Za-z0-9]+)(?:\.[a-zA-Z]{2,4})?$").unwrap();
    static ref PROPER_RE: Regex = Regex::new(r"(?i)\bPROPER\b").unwrap();
    static ref REPACK_RE: Regex = Regex::new(r"(?i)\bREPACK\b").unwrap();

    // Music-specific patterns
    static ref AUDIO_FORMAT_RE: Regex = Regex::new(r"(?i)\b(FLAC|MP3|AAC|ALAC|WAV|OGG|OPUS|APE)\b").unwrap();
    static ref BITRATE_RE: Regex = Regex::new(r"(?i)\b(320|256|192|128|V0|V1|V2)\s*k?(?:bps)?\b").unwrap();
    static ref SAMPLE_RATE_RE: Regex = Regex::new(r"(?i)(44\.1|48|96|192)\s*kHz").unwrap();
    static ref BIT_DEPTH_RE: Regex = Regex::new(r"(?i)(16|24|32)\s*(?:-)?bit").unwrap();
    static ref MUSIC_SOURCE_RE: Regex = Regex::new(r"(?i)\b(CD|WEB|Vinyl|Cassette|DAT|SACD|DVD-A)\b").unwrap();

    // Title extraction (for movies - stop at year or quality indicators)
    static ref TITLE_RE: Regex = Regex::new(r"^(.+?)(?:\.|_|-|\s)(?:\d{4}|S\d{1,2}E\d{1,2}|2160p|1080p|720p|480p)").unwrap();
}

/// Video quality resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Quality {
    #[serde(rename = "2160p")]
    P2160,
    #[serde(rename = "1080p")]
    P1080,
    #[serde(rename = "720p")]
    P720,
    #[serde(rename = "480p")]
    P480,
    #[serde(rename = "unknown")]
    Unknown,
}

impl Quality {
    /// Parse quality from string representation.
    pub fn from_str(s: &str) -> Quality {
        match s.to_lowercase().as_str() {
            "2160p" => Quality::P2160,
            "1080p" => Quality::P1080,
            "720p" => Quality::P720,
            "480p" => Quality::P480,
            _ => Quality::Unknown,
        }
    }

    /// Get a numerical score for quality comparison (higher is better).
    pub fn score(&self) -> u32 {
        match self {
            Quality::P2160 => 4,
            Quality::P1080 => 3,
            Quality::P720 => 2,
            Quality::P480 => 1,
            Quality::Unknown => 0,
        }
    }
}

/// Video source type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Source {
    BluRay,
    #[serde(rename = "WEB-DL")]
    WebDl,
    WebRip,
    Hdtv,
    DvdRip,
    Cam,
    Screener,
    Unknown,
}

impl Source {
    /// Parse source from string representation.
    pub fn from_str(s: &str) -> Source {
        match s.to_uppercase().as_str() {
            "BLURAY" | "BLU-RAY" | "BDRIP" | "BRRIP" => Source::BluRay,
            "WEB-DL" | "WEBDL" | "WEB" => Source::WebDl,
            "WEBRIP" => Source::WebRip,
            "HDTV" => Source::Hdtv,
            "DVDRIP" | "DVD" => Source::DvdRip,
            "CAM" | "TS" | "TELESYNC" | "HDCAM" => Source::Cam,
            "SCR" | "SCREENER" => Source::Screener,
            _ => Source::Unknown,
        }
    }

    /// Get a numerical score for source quality (higher is better).
    pub fn score(&self) -> u32 {
        match self {
            Source::BluRay => 5,
            Source::WebDl => 4,
            Source::WebRip => 3,
            Source::Hdtv => 3,
            Source::DvdRip => 2,
            Source::Screener => 1,
            Source::Cam => 0,
            Source::Unknown => 0,
        }
    }
}

/// Audio format for music releases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[allow(clippy::upper_case_acronyms)]
pub enum AudioFormat {
    Flac,
    Mp3,
    Aac,
    Alac,
    Wav,
    Ogg,
    Opus,
    Ape,
    Unknown,
}

impl AudioFormat {
    /// Parse audio format from string representation.
    pub fn from_str(s: &str) -> AudioFormat {
        match s.to_uppercase().as_str() {
            "FLAC" => AudioFormat::Flac,
            "MP3" => AudioFormat::Mp3,
            "AAC" => AudioFormat::Aac,
            "ALAC" => AudioFormat::Alac,
            "WAV" => AudioFormat::Wav,
            "OGG" => AudioFormat::Ogg,
            "OPUS" => AudioFormat::Opus,
            "APE" => AudioFormat::Ape,
            _ => AudioFormat::Unknown,
        }
    }

    /// Check if this is a lossless format.
    #[allow(dead_code)]
    pub fn is_lossless(&self) -> bool {
        matches!(
            self,
            AudioFormat::Flac | AudioFormat::Alac | AudioFormat::Wav | AudioFormat::Ape
        )
    }
}

/// Music source type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum MusicSource {
    Cd,
    Web,
    Vinyl,
    Cassette,
    Unknown,
}

impl MusicSource {
    /// Parse music source from string representation.
    pub fn from_str(s: &str) -> MusicSource {
        match s.to_uppercase().as_str() {
            "CD" | "SACD" | "DVD-A" => MusicSource::Cd,
            "WEB" => MusicSource::Web,
            "VINYL" => MusicSource::Vinyl,
            "CASSETTE" | "DAT" => MusicSource::Cassette,
            _ => MusicSource::Unknown,
        }
    }
}

/// Parsed release information extracted from a release name.
#[derive(Debug, Clone, Serialize)]
pub struct ParsedRelease {
    /// Extracted title (movie/show name)
    pub title: String,
    /// Release year
    pub year: Option<i32>,
    /// Season number (for TV shows)
    pub season: Option<i32>,
    /// Episode number (for TV shows)
    pub episode: Option<i32>,
    /// Video quality (resolution)
    pub quality: Quality,
    /// Video source type
    pub source: Source,
    /// Video codec
    pub codec: Option<String>,
    /// Audio codec/format
    pub audio: Option<String>,
    /// Release group name
    pub group: Option<String>,
    /// Whether this is a PROPER release
    pub proper: bool,
    /// Whether this is a REPACK release
    pub repack: bool,
    // Music-specific fields
    /// Artist name (for music)
    pub artist: Option<String>,
    /// Album name (for music)
    pub album: Option<String>,
    /// Audio format (for music)
    pub audio_format: Option<AudioFormat>,
    /// Bitrate in kbps (for music)
    pub bitrate: Option<u32>,
    /// Sample rate (for music)
    pub sample_rate: Option<String>,
    /// Bit depth (for music)
    pub bit_depth: Option<u32>,
    /// Music source
    pub music_source: Option<MusicSource>,
}

impl Default for ParsedRelease {
    fn default() -> Self {
        Self {
            title: String::new(),
            year: None,
            season: None,
            episode: None,
            quality: Quality::Unknown,
            source: Source::Unknown,
            codec: None,
            audio: None,
            group: None,
            proper: false,
            repack: false,
            artist: None,
            album: None,
            audio_format: None,
            bitrate: None,
            sample_rate: None,
            bit_depth: None,
            music_source: None,
        }
    }
}

/// Parse a video release name and extract quality information.
///
/// # Example
/// ```ignore
/// let parsed = parse_release_name("Movie.2024.1080p.BluRay.x264-GROUP");
/// assert_eq!(parsed.year, Some(2024));
/// assert_eq!(parsed.quality, Quality::P1080);
/// assert_eq!(parsed.source, Source::BluRay);
/// ```
pub fn parse_release_name(name: &str) -> ParsedRelease {
    let mut result = ParsedRelease::default();

    // Extract title
    if let Some(caps) = TITLE_RE.captures(name) {
        result.title = caps[1].replace(['.', '_'], " ").trim().to_string();
    } else {
        // Fallback: take everything before the first bracket or quality indicator
        let clean_name = name.replace(['.', '_'], " ");
        if let Some(pos) = clean_name.find(['[', '(']) {
            result.title = clean_name[..pos].trim().to_string();
        } else {
            result.title = clean_name
                .split_whitespace()
                .take(3)
                .collect::<Vec<_>>()
                .join(" ");
        }
    }

    // Extract year
    if let Some(caps) = YEAR_RE.captures(name) {
        if let Ok(year) = caps[1].parse::<i32>() {
            if (1900..=2100).contains(&year) {
                result.year = Some(year);
            }
        }
    }

    // Extract season and episode
    if let Some(caps) = SEASON_EP_RE.captures(name) {
        result.season = caps[1].parse().ok();
        result.episode = caps[2].parse().ok();
    }

    // Extract quality
    if let Some(caps) = QUALITY_RE.captures(name) {
        result.quality = Quality::from_str(&caps[1]);
    }

    // Extract source
    if let Some(caps) = SOURCE_RE.captures(name) {
        result.source = Source::from_str(&caps[1]);
    }

    // Extract codec
    if let Some(caps) = CODEC_RE.captures(name) {
        result.codec = Some(caps[1].to_uppercase());
    }

    // Extract audio
    if let Some(caps) = AUDIO_RE.captures(name) {
        result.audio = Some(caps[1].to_uppercase());
    }

    // Extract group
    if let Some(caps) = GROUP_RE.captures(name) {
        result.group = Some(caps[1].to_string());
    }

    // Check for PROPER/REPACK
    result.proper = PROPER_RE.is_match(name);
    result.repack = REPACK_RE.is_match(name);

    result
}

/// Parse a music release name and extract audio quality information.
///
/// Music releases typically follow formats like:
/// - "Artist - Album (2024) [FLAC]"
/// - "Artist - Album - 2024 - CD - FLAC"
/// - "Artist - Album (2024) [WEB] [FLAC] [24bit-96kHz]"
pub fn parse_music_release(name: &str) -> ParsedRelease {
    let mut result = ParsedRelease::default();

    // Try to extract artist - album pattern
    let clean_name = name.replace(['[', ']', '(', ')', '_'], " ");

    // Common pattern: "Artist - Album"
    if clean_name.contains(" - ") {
        let parts: Vec<&str> = clean_name.splitn(2, " - ").collect();
        if parts.len() == 2 {
            result.artist = Some(parts[0].trim().to_string());
            // Album might contain year and other info, try to clean it
            let album_part = parts[1];
            // Take until we hit a year pattern or quality indicator
            let album = album_part
                .split(|c: char| c.is_ascii_digit())
                .next()
                .unwrap_or(album_part)
                .trim()
                .trim_end_matches('-')
                .trim()
                .to_string();
            if !album.is_empty() {
                result.album = Some(album);
            }
        }
    }

    // Extract year
    if let Some(caps) = YEAR_RE.captures(name) {
        if let Ok(year) = caps[1].parse::<i32>() {
            if (1900..=2100).contains(&year) {
                result.year = Some(year);
            }
        }
    }

    // Also try simple year pattern for music (just 4 digits)
    if result.year.is_none() {
        let year_simple = Regex::new(r"\b(\d{4})\b").unwrap();
        if let Some(caps) = year_simple.captures(name) {
            if let Ok(year) = caps[1].parse::<i32>() {
                if (1950..=2100).contains(&year) {
                    result.year = Some(year);
                }
            }
        }
    }

    // Extract audio format
    if let Some(caps) = AUDIO_FORMAT_RE.captures(name) {
        result.audio_format = Some(AudioFormat::from_str(&caps[1]));
    }

    // Extract bitrate
    if let Some(caps) = BITRATE_RE.captures(name) {
        let bitrate_str = caps[1].to_uppercase();
        result.bitrate = match bitrate_str.as_str() {
            "320" => Some(320),
            "256" => Some(256),
            "192" => Some(192),
            "128" => Some(128),
            "V0" => Some(245), // VBR V0 averages around 245kbps
            "V1" => Some(225),
            "V2" => Some(190),
            _ => None,
        };
    }

    // Extract sample rate
    if let Some(caps) = SAMPLE_RATE_RE.captures(name) {
        result.sample_rate = Some(format!("{}kHz", &caps[1]));
    }

    // Extract bit depth
    if let Some(caps) = BIT_DEPTH_RE.captures(name) {
        result.bit_depth = caps[1].parse().ok();
    }

    // Extract music source
    if let Some(caps) = MUSIC_SOURCE_RE.captures(name) {
        result.music_source = Some(MusicSource::from_str(&caps[1]));
    }

    // Extract group (if present)
    if let Some(caps) = GROUP_RE.captures(name) {
        result.group = Some(caps[1].to_string());
    }

    // Set title from artist + album
    if let (Some(artist), Some(album)) = (&result.artist, &result.album) {
        result.title = format!("{} - {}", artist, album);
    } else if let Some(artist) = &result.artist {
        result.title = artist.clone();
    } else {
        result.title = name.to_string();
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_movie_release() {
        let parsed = parse_release_name("Movie.2024.1080p.BluRay.x264-GROUP");
        assert_eq!(parsed.year, Some(2024));
        assert_eq!(parsed.quality, Quality::P1080);
        assert_eq!(parsed.source, Source::BluRay);
        assert_eq!(parsed.codec, Some("X264".to_string()));
        assert_eq!(parsed.group, Some("GROUP".to_string()));
    }

    #[test]
    fn test_parse_tv_release() {
        let parsed = parse_release_name("Show.Name.S01E05.1080p.WEB-DL.AAC-GROUP");
        assert_eq!(parsed.season, Some(1));
        assert_eq!(parsed.episode, Some(5));
        assert_eq!(parsed.quality, Quality::P1080);
        assert_eq!(parsed.source, Source::WebDl);
        assert_eq!(parsed.audio, Some("AAC".to_string()));
    }

    #[test]
    fn test_parse_4k_release() {
        let parsed = parse_release_name("Movie.2023.2160p.WEB-DL.x265.HEVC.DTS-GROUP");
        assert_eq!(parsed.quality, Quality::P2160);
        assert_eq!(parsed.source, Source::WebDl);
        assert_eq!(parsed.codec, Some("X265".to_string()));
        assert_eq!(parsed.audio, Some("DTS".to_string()));
    }

    #[test]
    fn test_parse_proper_repack() {
        let parsed = parse_release_name("Movie.2024.1080p.BluRay.PROPER-GROUP");
        assert!(parsed.proper);
        assert!(!parsed.repack);

        let parsed = parse_release_name("Movie.2024.1080p.BluRay.REPACK-GROUP");
        assert!(!parsed.proper);
        assert!(parsed.repack);
    }

    #[test]
    fn test_parse_music_release() {
        let parsed = parse_music_release("Artist Name - Album Title (2024) [FLAC]");
        assert_eq!(parsed.artist, Some("Artist Name".to_string()));
        assert_eq!(parsed.album, Some("Album Title".to_string()));
        assert_eq!(parsed.year, Some(2024));
        assert_eq!(parsed.audio_format, Some(AudioFormat::Flac));
    }

    #[test]
    fn test_parse_music_with_bitrate() {
        let parsed = parse_music_release("Artist - Album (2023) [MP3 320kbps]");
        assert_eq!(parsed.audio_format, Some(AudioFormat::Mp3));
        assert_eq!(parsed.bitrate, Some(320));
    }

    #[test]
    fn test_parse_music_high_res() {
        let parsed = parse_music_release("Artist - Album [2024] [FLAC] [24bit-96kHz]");
        assert_eq!(parsed.audio_format, Some(AudioFormat::Flac));
        assert_eq!(parsed.bit_depth, Some(24));
        assert_eq!(parsed.sample_rate, Some("96kHz".to_string()));
    }

    #[test]
    fn test_quality_scores() {
        assert!(Quality::P2160.score() > Quality::P1080.score());
        assert!(Quality::P1080.score() > Quality::P720.score());
        assert!(Quality::P720.score() > Quality::P480.score());
    }

    #[test]
    fn test_source_scores() {
        assert!(Source::BluRay.score() > Source::WebDl.score());
        assert!(Source::WebDl.score() > Source::Cam.score());
    }

    #[test]
    fn test_audio_format_lossless() {
        assert!(AudioFormat::Flac.is_lossless());
        assert!(AudioFormat::Alac.is_lossless());
        assert!(!AudioFormat::Mp3.is_lossless());
        assert!(!AudioFormat::Aac.is_lossless());
    }
}
