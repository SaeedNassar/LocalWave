//! Faithful port of `server/src/services/lyricsService.ts`.
//! Fetches lyrics from Musixmatch first (if token configured), then lrclib.net,
//! with SQLite caching.
//!
//! Important Rust-specific note: `rusqlite::Connection` is `!Sync`, so a
//! `&Connection` held across an `.await` makes the future `!Send`. To keep
//! handlers `Send` (required by axum's multi-threaded runtime), DB reads are
//! performed and the connection dropped *before* any network await; a fresh
//! connection is acquired for the cache write afterwards.

use once_cell::sync::Lazy;
use regex::Regex;
use rusqlite::params;
use serde::Deserialize;

use crate::config::load_config;
use crate::db::{get_conn, DbPool};
use crate::musixmatch::get_musixmatch_lyrics;
use crate::types::{LyricLine, LyricsPayload};

#[derive(Debug, Deserialize)]
struct LrclibEntry {
    #[serde(rename = "duration")]
    duration: Option<f64>,
    #[serde(rename = "syncedLyrics")]
    synced_lyrics: Option<String>,
    #[serde(rename = "plainLyrics")]
    plain_lyrics: Option<String>,
}

pub async fn get_lyrics_for_track(pool: &DbPool, track_id: i64) -> Option<LyricsPayload> {
    let cfg = load_config().ok()?;
    if !cfg.enable_lyrics {
        return None;
    }

    // 1. Check DB cache first (release connection before any await)
    let cache_or_track = {
        let conn = get_conn(pool).ok()?;
        let cached = conn
            .query_row(
                "SELECT lyrics, synced FROM lyrics_cache WHERE track_id = ?",
                [track_id],
                |row| {
                    Ok(LyricsPayload {
                        lyrics: row.get::<usize, String>(0)?,
                        synced: row.get::<usize, i64>(1)? == 1,
                    })
                },
            )
            .ok();
        if let Some(c) = cached {
            Some(Either::Cached(c))
        } else {
            // 2. Get track metadata for the query
            conn.query_row(
                "SELECT title, artist, duration FROM tracks WHERE id = ?",
                [track_id],
                |row| {
                    Ok((
                        row.get::<usize, String>(0)?,
                        row.get::<usize, Option<String>>(1)?,
                        row.get::<usize, f64>(2)?,
                    ))
                },
            )
            .ok()
            .map(Either::Track)
        }
    };

    // Return cached immediately if present
    if let Some(Either::Cached(c)) = cache_or_track {
        return Some(c);
    }

    let (title, artist, duration) = match cache_or_track {
        Some(Either::Track(t)) => t,
        _ => return None,
    };

    // 3. Try Musixmatch first when a token is configured (subtitle lyrics are always synced)
    if !cfg.musixmatch_access_token.is_empty() {
        if let Some(mm_result) = get_musixmatch_lyrics(
            &cfg.musixmatch_access_token,
            &title,
            artist.as_deref(),
            duration,
        )
        .await
        {
            let payload = LyricsPayload {
                lyrics: mm_result.lyrics,
                synced: mm_result.synced,
            };
            cache_lyrics(pool, track_id, Some(&payload));
            return Some(payload);
        }
    }

    // 4. Fallback to lrclib.net
    if let Some(result) = fetch_lrclib_lyrics(&title, artist.as_deref(), duration).await {
        cache_lyrics(pool, track_id, Some(&result));
        return Some(result);
    }

    // Nothing found — store empty cache so we don't retry constantly
    cache_lyrics(pool, track_id, None);
    None
}

enum Either {
    Cached(LyricsPayload),
    Track((String, Option<String>, f64)),
}

async fn fetch_lrclib_lyrics(
    title: &str,
    artist: Option<&str>,
    duration: f64,
) -> Option<LyricsPayload> {
    let query = format!("{} {}", title, artist.unwrap_or("")).trim().to_string();
    if query.is_empty() {
        return None;
    }

    let url = format!("https://lrclib.net/api/search?q={}", urlencoding::encode(&query));
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .ok()?;

    let max_retries = 3u32;
    let mut last_err = None;
    for attempt in 0..max_retries {
        match client
            .get(&url)
            .header("Accept", "application/json")
            .send()
            .await
        {
            Ok(response) => {
                if !response.status().is_success() {
                    last_err = Some(format!("HTTP {}", response.status()));
                    if attempt + 1 < max_retries {
                        tokio::time::sleep(std::time::Duration::from_millis(
                            (2_u64.pow(attempt + 1)) * 1000,
                        ))
                        .await;
                        continue;
                    }
                    return None;
                }

                let entries: Vec<LrclibEntry> = match response.json().await {
                    Ok(v) => v,
                    Err(e) => {
                        last_err = Some(e.to_string());
                        if attempt + 1 < max_retries {
                            tokio::time::sleep(std::time::Duration::from_millis(
                                (2_u64.pow(attempt + 1)) * 1000,
                            ))
                            .await;
                            continue;
                        }
                        return None;
                    }
                };

                return parse_lrclib_entries(&entries, duration);
            }
            Err(e) => {
                last_err = Some(e.to_string());
                if attempt + 1 < max_retries {
                    tokio::time::sleep(std::time::Duration::from_millis(
                        (2_u64.pow(attempt + 1)) * 1000,
                    ))
                    .await;
                    continue;
                }
            }
        }
    }

    if let Some(e) = last_err {
        eprintln!("[lyrics] lrclib fetch failed after retries: {}", e);
    }
    None
}

fn parse_lrclib_entries(entries: &[LrclibEntry], duration: f64) -> Option<LyricsPayload> {
    if entries.is_empty() {
        return None;
    }

    // Filter by +/-5s duration match
    let matched: Vec<&LrclibEntry> = entries
        .iter()
        .filter(|e| {
            let d = e.duration.unwrap_or(0.0);
            (d - duration).abs() <= 5.0 && (e.synced_lyrics.is_some() || e.plain_lyrics.is_some())
        })
        .collect();
    let candidates = if !matched.is_empty() {
        matched
    } else {
        entries
            .iter()
            .filter(|e| e.synced_lyrics.is_some() || e.plain_lyrics.is_some())
            .collect()
    };
    if candidates.is_empty() {
        return None;
    }

    // Prefer synced
    let mut sorted = candidates.clone();
    sorted.sort_by(|a, b| {
        let a_synced = a.synced_lyrics.is_some();
        let b_synced = b.synced_lyrics.is_some();
        b_synced.cmp(&a_synced)
    });

    let best = sorted.first()?;
    let lyrics = best.synced_lyrics.clone().or_else(|| best.plain_lyrics.clone())?;
    Some(LyricsPayload {
        lyrics,
        synced: best.synced_lyrics.is_some(),
    })
}

fn cache_lyrics(pool: &DbPool, track_id: i64, result: Option<&LyricsPayload>) {
    if let Ok(conn) = get_conn(pool) {
        let _ = conn.execute(
            "INSERT OR REPLACE INTO lyrics_cache (track_id, lyrics, synced, fetched_at) VALUES (?, ?, ?, ?)",
            params![
                track_id,
                result.map(|r| r.lyrics.as_str()).unwrap_or(""),
                result.map(|r| r.synced).unwrap_or(false) as i32,
                chrono::Utc::now().to_rfc3339(),
            ],
        );
    }
}

static LRC_LINE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\[(\d{2}):(\d{2}\.\d{2,3})\]\s*(.*)$").unwrap()
});
static SYNCED_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\[\d{2}:\d{2}\.\d{2,3}\]").unwrap()
});

pub fn is_synced_lrc(lyrics: &str) -> bool {
    SYNCED_RE.is_match(lyrics)
}

pub fn parse_lrc(lyrics: &str) -> Vec<LyricLine> {
    lyrics
        .lines()
        .map(|line| {
            if let Some(caps) = LRC_LINE_RE.captures(line) {
                let minutes: i64 = caps[1].parse().ok()?;
                let seconds: f64 = caps[2].parse().ok()?;
                let text = caps.get(3).map(|m| m.as_str()).unwrap_or("…");
                Some(LyricLine {
                    time: (minutes as f64) * 60.0 + seconds,
                    text: if text.is_empty() { "…".to_string() } else { text.to_string() },
                })
            } else {
                Some(LyricLine {
                    time: -1.0,
                    text: line.to_string(),
                })
            }
        })
        .filter_map(|opt| opt)
        .filter(|l| !l.text.trim().is_empty())
        .collect()
}
