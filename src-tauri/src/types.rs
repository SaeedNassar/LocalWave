//! Shared data types. serde camelCase so JSON matches the original TS frontend
//! shapes byte-for-byte (TrackArtist, Track, Album, Artist, Playlist, ...).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackArtist {
    pub id: i64,
    pub name: String,
    pub role: String,
    pub position: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub id: i64,
    pub path: String,
    pub filename: String,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    #[serde(rename = "albumArtist")]
    pub album_artist: Option<String>,
    #[serde(rename = "albumId")]
    pub album_id: Option<i64>,
    #[serde(rename = "artistId")]
    pub artist_id: Option<i64>,
    pub duration: f64,
    #[serde(rename = "trackNumber")]
    pub track_number: Option<i64>,
    #[serde(rename = "diskNumber")]
    pub disk_number: Option<i64>,
    pub format: Option<String>,
    pub bitrate: Option<i64>,
    #[serde(rename = "sampleRate")]
    pub sample_rate: Option<i64>,
    #[serde(rename = "hasCover")]
    pub has_cover: bool,
    pub liked: bool,
    #[serde(rename = "playCount")]
    pub play_count: i64,
    #[serde(rename = "dateAdded")]
    pub date_added: String,
    #[serde(rename = "fileModifiedAt")]
    pub file_modified_at: Option<String>,
    pub artists: Vec<TrackArtist>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Album {
    pub id: i64,
    pub name: String,
    #[serde(rename = "albumArtist")]
    pub album_artist: Option<String>,
    #[serde(rename = "artistId")]
    pub artist_id: Option<i64>,
    pub year: Option<i64>,
    #[serde(rename = "trackCount")]
    pub track_count: i64,
    #[serde(rename = "hasCover")]
    pub has_cover: bool,
    #[serde(rename = "coverTrackId")]
    pub cover_track_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artist {
    pub id: i64,
    pub name: String,
    #[serde(rename = "albumCount")]
    pub album_count: i64,
    #[serde(rename = "trackCount")]
    pub track_count: i64,
    #[serde(rename = "primaryTrackCount", skip_serializing_if = "Option::is_none")]
    pub primary_track_count: Option<i64>,
    #[serde(rename = "imagePath", skip_serializing_if = "Option::is_none")]
    pub image_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Playlist {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    #[serde(rename = "trackCount")]
    pub track_count: i64,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "isImported")]
    pub is_imported: bool,
    pub source: Option<String>,
    pub position: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistEntry {
    #[serde(rename = "playlistId")]
    pub playlist_id: i64,
    pub position: i64,
    #[serde(rename = "trackId")]
    pub track_id: Option<i64>,
    #[serde(rename = "rawEntry")]
    pub raw_entry: String,
    pub missing: bool,
    pub title: Option<String>,
    pub track: Option<Track>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistDetail {
    #[serde(flatten)]
    pub playlist: Playlist,
    pub entries: Vec<PlaylistEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanStatus {
    pub tracks: i64,
    pub albums: i64,
    pub artists: i64,
    pub playlists: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgress {
    pub scanned: i64,
    pub added: i64,
    pub updated: i64,
    pub failed: i64,
    #[serde(rename = "playlistsImported")]
    pub playlists_imported: i64,
}

// ── config ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    #[serde(rename = "musicFolder")]
    pub music_folder: String,
    #[serde(rename = "supportedExtensions")]
    pub supported_extensions: Vec<String>,
    #[serde(rename = "scanIntervalMs")]
    pub scan_interval_ms: u64,
    pub port: u16,
    #[serde(rename = "spDc")]
    pub sp_dc: String,
    #[serde(rename = "enableLyrics")]
    pub enable_lyrics: bool,
    #[serde(rename = "enableCanvas")]
    pub enable_canvas: bool,
    #[serde(rename = "musixmatchAccessToken")]
    pub musixmatch_access_token: String,
}

// ── enrichment / misc response shapes ───────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricLine {
    pub time: f64,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsResult {
    pub lyrics: String,
    pub synced: bool,
    pub lines: Vec<LyricLine>,
}

#[derive(Debug, Clone)]
pub struct LyricsPayload {
    pub lyrics: String,
    pub synced: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistWithRole {
    pub id: i64,
    pub name: String,
    #[serde(rename = "imagePath")]
    pub image_path: Option<String>,
    pub role: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Features {
    pub lyrics: bool,
    pub canvas: bool,
    #[serde(rename = "hasSpDc")]
    pub has_sp_dc: bool,
    #[serde(rename = "musixmatchAccessToken")]
    pub musixmatch_access_token: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackMetadata {
    pub title: String,
    pub artist: String,
    pub album: String,
    #[serde(rename = "albumArtist")]
    pub album_artist: String,
    pub year: Option<i64>,
    #[serde(rename = "trackNumber")]
    pub track_number: Option<i64>,
    #[serde(rename = "coverArt")]
    pub cover_art: Option<CoverArt>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverArt {
    #[serde(rename = "mimeType")]
    pub mime_type: String,
    pub data: String, // base64
}
