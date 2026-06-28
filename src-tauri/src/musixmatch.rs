//! Faithful port of `server/src/services/musixmatchLyricsService.ts`.
//! Searches Musixmatch for a track with subtitles, then fetches the subtitle body.

use std::time::Duration;

use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MusixmatchLyricsResult {
    pub lyrics: String,
    pub synced: bool,
}

#[derive(Debug, Deserialize)]
struct MusixmatchTrack {
    track_id: i64,
    #[allow(dead_code)]
    track_name: String,
    #[allow(dead_code)]
    artist_name: String,
    track_length: f64,
}

#[derive(Debug, Deserialize)]
struct MusixmatchResponseHeader {
    status_code: i32,
    #[allow(dead_code)]
    hint: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MusixmatchResponseMessage<T> {
    header: MusixmatchResponseHeader,
    body: T,
}

#[derive(Debug, Deserialize)]
struct MusixmatchResponse<T> {
    message: MusixmatchResponseMessage<T>,
}

#[derive(Debug, Deserialize)]
struct TrackListWrapper {
    track: MusixmatchTrack,
}

#[derive(Debug, Deserialize)]
struct TrackSearchBody {
    track_list: Vec<TrackListWrapper>,
}

#[derive(Debug, Deserialize)]
struct SubtitleBody {
    subtitle_body: String,
}

#[derive(Debug, Deserialize)]
struct SubtitleWrapper {
    subtitle: Option<SubtitleBody>,
}

pub async fn get_musixmatch_lyrics(
    access_token: &str,
    track_title: &str,
    track_artist: Option<&str>,
    duration_seconds: f64,
) -> Option<MusixmatchLyricsResult> {
    if access_token.is_empty() {
        return None;
    }

    let q_track = urlencoding::encode(track_title);
    let q_artist = urlencoding::encode(track_artist.unwrap_or(""));
    let q_duration = duration_seconds.round() as i64;

    // Step 1: Find Track ID
    let search_url = format!(
        "https://api.musixmatch.com/ws/1.1/track.search?q_track={}&q_artist={}&f_has_subtitle=1&q_duration={}&s_track_rating=desc&page_size=5&apikey={}",
        q_track, q_artist, q_duration, access_token
    );

    let client = Client::builder()
        .timeout(Duration::from_millis(10000))
        .build()
        .ok()?;

    let track_id: i64 = (|| async {
        let res = client
            .get(&search_url)
            .header("Accept", "application/json")
            .send()
            .await
            .ok()?;
        if !res.status().is_success() {
            return None;
        }
        let search_data: MusixmatchResponse<TrackSearchBody> = res.json().await.ok()?;
        if search_data.message.header.status_code != 200 {
            return None;
        }
        let candidates: Vec<MusixmatchTrack> = search_data
            .message
            .body
            .track_list
            .into_iter()
            .map(|w| w.track)
            .collect();
        let matched = candidates
            .iter()
            .find(|t| (t.track_length - duration_seconds).abs() <= 5.0);
        matched
            .or_else(|| candidates.first())
            .map(|t| t.track_id)
    })()
    .await?;

    // Step 2: Fetch subtitle
    let subtitle_url = format!(
        "https://api.musixmatch.com/ws/1.1/track.subtitle.get?track_id={}&apikey={}",
        track_id, access_token
    );

    (|| async {
        let res = client
            .get(&subtitle_url)
            .header("Accept", "application/json")
            .send()
            .await
            .ok()?;
        if !res.status().is_success() {
            return None;
        }
        let data: MusixmatchResponse<SubtitleWrapper> = res.json().await.ok()?;
        if data.message.header.status_code != 200 {
            return None;
        }
        let subtitle_body = data.message.body.subtitle?.subtitle_body;
        let trimmed = subtitle_body.trim();
        if trimmed.is_empty() {
            return None;
        }
        Some(MusixmatchLyricsResult {
            lyrics: subtitle_body,
            synced: true,
        })
    })()
    .await
}

/// Store a Musixmatch result in the same SQLite cache shape as other lyrics.
/// Ported from the original `cacheMusixmatchLyrics` function.
pub fn cache_musixmatch_lyrics(
    conn: &mut rusqlite::Connection,
    track_id: i64,
    result: Option<&MusixmatchLyricsResult>,
) -> Result<(), rusqlite::Error> {
    let lyrics = result.map(|r| r.lyrics.as_str()).unwrap_or("");
    let synced = result.map(|r| r.synced as i64).unwrap_or(0);
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT OR REPLACE INTO lyrics_cache (track_id, lyrics, synced, fetched_at) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![track_id, lyrics, synced, now],
    )?;
    Ok(())
}
