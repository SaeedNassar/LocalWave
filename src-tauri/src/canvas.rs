//! Faithful port of `server/src/services/canvasService.ts`.
//! Looks up a Spotify Canvas video URL for a local track, with SQLite caching.

use std::collections::HashMap;

use prost::Message;
use reqwest::Method;
use serde_json::json;

use crate::config::load_config;
use crate::db::{get_conn, DbError, DbPool};
use crate::proto_canvas::{CanvasRequest, CanvasResponse};
use crate::spotify_api_helper::{axios_retry_bytes, axios_retry_json, ApiError};
use crate::spotify_auth::get_tokens;

const GRAPHQL_URL: &str = "https://api-partner.spotify.com/pathfinder/v2/query";
const SEARCH_HASH: &str = "e0ec36bbc74e39d1787cbe8ee2939cf6ef55edd3535572521bc62b3e4159ba0d";
const CACHE_TTL_MS: i64 = 7 * 24 * 60 * 60 * 1000; // 7 days

#[derive(Debug, Clone)]
pub struct CanvasResult {
    pub url: String,
    pub artist_uri: Option<String>,
    pub artist_name: Option<String>,
    pub artist_img_url: Option<String>,
}

async fn extract_track_id(data: &serde_json::Value, track_name: &str, artist_name: &str) -> Option<String> {
    let items = data
        .get("searchV2")
        .and_then(|v| v.get("topResultsV2"))
        .and_then(|v| v.get("itemsV2"))
        .or_else(|| {
            data.get("data")
                .and_then(|v| v.get("searchV2"))
                .and_then(|v| v.get("topResultsV2"))
                .and_then(|v| v.get("itemsV2"))
        })
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let track_name_lower = track_name.to_lowercase();
    let artist_name_lower = artist_name.to_lowercase();

    let mut first_track_fallback: Option<String> = None;

    for item in &items {
        // Only TrackResponseWrapper items are actual tracks.
        let wrapper_type = item
            .get("item")
            .and_then(|v| v.get("__typename"))
            .and_then(|v| v.as_str());

        if wrapper_type != Some("TrackResponseWrapper") {
            continue; // skips ArtistResponseWrapper, PlaylistResponseWrapper, AlbumResponseWrapper, etc.
        }

        let item_data = match item.get("item").and_then(|v| v.get("data")) {
            Some(d) => d,
            None => continue,
        };

        let raw_uri = match item_data.get("uri").and_then(|v| v.as_str()) {
            Some(u) => u,
            None => continue,
        };
        if !raw_uri.starts_with("spotify:track:") {
            continue;
        }
        let id = raw_uri.trim_start_matches("spotify:track:").to_string();

        if first_track_fallback.is_none() {
            first_track_fallback = Some(id.clone());
        }

        let item_title = item_data
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_lowercase();

        let item_artists: Vec<String> = item_data
            .get("artists")
            .and_then(|v| v.get("items"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|a| a.get("profile").and_then(|p| p.get("name")).and_then(|n| n.as_str()))
                    .map(|s| s.to_lowercase())
                    .collect()
            })
            .unwrap_or_default();

        let title_matches = item_title == track_name_lower
            || item_title.contains(&track_name_lower)
            || track_name_lower.contains(&item_title);
        let artist_matches = artist_name_lower.is_empty()
            || item_artists.iter().any(|a| a.contains(&artist_name_lower) || artist_name_lower.contains(a.as_str()));

        if title_matches && artist_matches {
            log::info!(
                "[canvas] Matched track locally: id={} title=\"{}\" artists={:?}",
                id, item_title, item_artists
            );
            return Some(id);
        } else {
            log::info!(
                "[canvas] Candidate id={} didn't match (title=\"{}\" artists={:?}) — checking next",
                id, item_title, item_artists
            );
        }
    }

    if let Some(fallback) = &first_track_fallback {
        log::warn!("[canvas] No exact local match — falling back to top track result: {}", fallback);
    } else {
        log::info!("[canvas] No track-type results in search response at all");
    }
    first_track_fallback
}

async fn search_track_id(
    track_name: &str,
    artist_name: &str,
    access_token: &str,
    client_token: &str,
) -> Result<Option<String>, ApiError> {
    let query = format!("{} {}", track_name, artist_name);
    let payload = json!({
        "variables": {
            "numberOfTopResults": 10,
            "offset": 0,
            "includeAuthors": false,
            "query": query,
            "limit": query.chars().count() + 8,
        },
        "operationName": "searchSuggestions",
        "extensions": {
            "persistedQuery": { "version": 1, "sha256Hash": SEARCH_HASH },
        }
    });

    let mut headers = HashMap::new();
    headers.insert("accept".to_string(), "application/json".to_string());
    headers.insert("content-type".to_string(), "application/json;charset=UTF-8".to_string());
    headers.insert("authorization".to_string(), format!("Bearer {}", access_token));
    headers.insert("client-token".to_string(), client_token.to_string());
    headers.insert("app-platform".to_string(), "WebPlayer".to_string());
    headers.insert("origin".to_string(), "https://open.spotify.com".to_string());
    headers.insert("referer".to_string(), "https://open.spotify.com/".to_string());

    // Bump retries so a single 429 doesn't immediately exhaust the budget;
    // backoff now respects Retry-After inside axios_retry_json.
    let res = axios_retry_json(
        GRAPHQL_URL,
        Method::POST,
        Some(&headers),
        Some(&payload),
        10000,
        4,
    )
    .await?;

    Ok(extract_track_id(&res.data, track_name, artist_name).await)
}

async fn fetch_canvas_url(track_id: &str, access_token: &str) -> Result<Option<CanvasResult>, ApiError> {
    let mut req = CanvasRequest::default();
    req.tracks.push(crate::proto_canvas::canvas_request::Track {
        track_uri: format!("spotify:track:{}", track_id),
    });
    let bytes = req.encode_to_vec();

    let mut headers = HashMap::new();
    headers.insert("Accept".to_string(), "application/protobuf".to_string());
    headers.insert("Content-Type".to_string(), "application/x-www-form-urlencoded".to_string());
    headers.insert("Accept-Language".to_string(), "en".to_string());
    headers.insert("User-Agent".to_string(), "Spotify/9.0.34.593 iOS/18.4 (iPhone15,3)".to_string());
    headers.insert("Authorization".to_string(), format!("Bearer {}", access_token));

    let res = axios_retry_bytes(
        "https://spclient.wg.spotify.com/canvaz-cache/v0/canvases",
        Method::POST,
        Some(&headers),
        Some(bytes),
        10000,
        3,
    )
    .await?;

    if res.status != 200 {
        return Ok(None);
    }

    let parsed = CanvasResponse::decode(&*res.data).map_err(|e| ApiError::Other(e.to_string()))?;
    let first = parsed.canvases.into_iter().next();
    match first {
        Some(c) if !c.canvas_url.is_empty() => {
            let artist = c.artist;
            Ok(Some(CanvasResult {
                url: c.canvas_url,
                artist_uri: artist.as_ref().map(|a| a.artist_uri.clone()),
                artist_name: artist.as_ref().map(|a| a.artist_name.clone()),
                artist_img_url: artist.as_ref().map(|a| a.artist_img_url.clone()),
            }))
        }
        _ => Ok(None),
    }
}

pub async fn get_canvas_for_track(pool: &DbPool, track_id: i64) -> Option<CanvasResult> {
    log::info!("[canvas] === Starting canvas fetch for track_id={} ===", track_id);

    let cfg = match load_config() {
        Ok(c) => c,
        Err(_) => { log::warn!("[canvas] Failed to load config"); return None; }
    };
    if !cfg.enable_canvas || cfg.sp_dc.is_empty() {
        log::info!("[canvas] Canvas disabled or no sp_dc — skipping");
        return None;
    }
    log::info!("[canvas] Config OK — enableCanvas={}, spDc length={}", cfg.enable_canvas, cfg.sp_dc.len());

    // 1. Read cache + track metadata (release connection before any await)
    log::info!("[canvas] Step 1: Reading cache + track metadata from DB...");
    let (cached, track_meta) = {
        let conn = get_conn(pool).ok()?;
        let cached: Option<(String, Option<String>, Option<String>, Option<String>, String)> = conn
            .query_row(
                "SELECT url, artist_uri, artist_name, artist_img_url, fetched_at FROM canvas_url_cache WHERE track_id = ?",
                [track_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                },
            )
            .ok();

        let track: Option<(String, Option<String>)> = conn
            .query_row(
                "SELECT title, artist FROM tracks WHERE id = ?",
                [track_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
            )
            .ok();

        (cached, track)
    };
    // conn dropped here — pool slot released

    let now = chrono::Utc::now();
    if let Some((url, artist_uri, artist_name, artist_img_url, fetched_at)) = cached {
        log::info!("[canvas] Cache HIT for track_id={} — url=\"{}\", fetched_at={}", track_id, url, fetched_at);
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&fetched_at) {
            let age_ms = now.timestamp_millis() - dt.timestamp_millis();
            let age_hours = age_ms / 3_600_000;
            log::info!("[canvas] Cache age: {} hours (TTL: {} hours)", age_hours, CACHE_TTL_MS / 3_600_000);
            if age_ms < CACHE_TTL_MS {
                if url.is_empty() {
                    log::info!("[canvas] Returning negative cache (no canvas for this track)");
                    return None;
                }
                log::info!("[canvas] Returning cached canvas: url={}", url);
                return Some(CanvasResult {
                    url,
                    artist_uri,
                    artist_name,
                    artist_img_url,
                });
            } else {
                log::info!("[canvas] Cache expired — will re-fetch from Spotify");
            }
        }
    } else {
        log::info!("[canvas] Cache MISS for track_id={}", track_id);
    }

    let (title, artist) = match track_meta {
        Some(t) => {
            log::info!("[canvas] Track found: title=\"{}\", artist=\"{}\"", t.0, t.1.as_deref().unwrap_or("(none)"));
            t
        }
        None => {
            log::warn!("[canvas] Track not found in DB for id={}", track_id);
            return None;
        }
    };

    // 2. Fetch tokens (network — no DB connection held)
    log::info!("[canvas] Step 2: Fetching Spotify tokens...");
    let (access_token, client_token) = match get_tokens(Some(&cfg.sp_dc)).await {
        Ok(t) => {
            log::info!("[canvas] Tokens obtained — access_token length={}, client_token length={}", t.0.len(), t.1.len());
            log::info!("[canvas] access_token (first 30 chars): {}...", &t.0[..t.0.len().min(30)]);
            log::info!("[canvas] client_token (first 30 chars): {}...", &t.1[..t.1.len().min(30)]);
            t
        }
        Err(e) => {
            log::error!("[canvas] Token fetch FAILED: {}", e);
            return None;
        }
    };

    // 3. Search track ID (network — no DB connection held)
    let query = format!("{} {}", artist.as_deref().unwrap_or(""), title);
    log::info!("[canvas] Step 3: Searching Spotify for: \"{}\"", query);
	let spotify_track_id = match search_track_id(&title,
			artist.as_deref().unwrap_or(""),
			&access_token,
			&client_token,
	)
	.await
	{
		Ok(Some(id)) => {
			log::info!("[canvas] Spotify track ID found: {} (uri: spotify:track:{})", id, id);
			id
		}
		Ok(None) => {
			log::info!("[canvas] No Spotify track found in search results — caching negative");
			cache_negative(pool, track_id);
			return None;
		}
		Err(ApiError::RateLimited { retry_after_ms }) => {
			log::warn!(
				"[canvas] Track search rate-limited (retry_after_ms={:?}) — NOT caching negative",
				retry_after_ms
			);
			return None;
		}
		Err(e) => {
			log::error!("[canvas] Track search FAILED: {}", e);
			return None;
		}
	};

    // 4. Fetch canvas URL (network — no DB connection held)
    let track_uri = format!("spotify:track:{}", spotify_track_id);
    log::info!("[canvas] Step 4: Fetching canvas for uri={}", track_uri);
	let result = match fetch_canvas_url(&spotify_track_id, &access_token).await {
        Ok(Some(r)) => {
            log::info!("[canvas] Canvas URL found: {}", r.url);
            log::info!("[canvas] Canvas artist: {:?} ({:?})", r.artist_name, r.artist_uri);
            r
        }
        Ok(None) => {
            log::info!("[canvas] No canvas returned for this track — caching negative");
            cache_negative(pool, track_id);
            return None;
        }
        Err(ApiError::RateLimited { retry_after_ms }) => {
            log::warn!(
                "[canvas] Canvas fetch rate-limited (retry_after_ms={:?}) — NOT caching negative",
                retry_after_ms
            );
            return None;
        }
        Err(e) => {
            log::error!("[canvas] Canvas fetch FAILED: {}", e);
            return None;
        }
    };

    // 5. Write cache (new connection, quick write)
    log::info!("[canvas] Step 5: Writing result to cache...");
    let fetched_at = now.to_rfc3339();
    if let Ok(conn) = get_conn(pool) {
        if let Err(e) = conn.execute(
            "INSERT OR REPLACE INTO canvas_url_cache (track_id, url, artist_uri, artist_name, artist_img_url, fetched_at) VALUES (?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                track_id,
                &result.url,
                result.artist_uri.as_deref(),
                result.artist_name.as_deref(),
                result.artist_img_url.as_deref(),
                fetched_at,
            ],
        ) {
            log::error!("[canvas] Cache write FAILED: {}", e);
        } else {
            log::info!("[canvas] Cache write OK");
        }
    }

    log::info!("[canvas] === Canvas fetch complete for track_id={} ===", track_id);
    Some(result)
}

fn cache_negative(pool: &DbPool, track_id: i64) {
    if let Ok(conn) = get_conn(pool) {
        let now = chrono::Utc::now().to_rfc3339();
        let _ = conn.execute(
            "INSERT OR REPLACE INTO canvas_url_cache (track_id, url, fetched_at) VALUES (?, '', ?)",
            rusqlite::params![track_id, now],
        );
    }
}

/// Clear canvas cache entries for tracks whose title + primary artist match
/// the given strings (case-insensitive). Returns how many cache rows were deleted.
pub fn clear_canvas_cache_by_title_artist(
    pool: &DbPool,
    title: &str,
    artist: &str,
) -> Result<usize, DbError> {
    let mut conn = get_conn(pool)?;
    let normalized_title = title.trim().to_lowercase();
    let normalized_artist = artist.trim().to_lowercase();

    let mut stmt = conn.prepare(
        "SELECT t.id
         FROM tracks t
         LEFT JOIN track_artists ta ON ta.track_id = t.id AND ta.role = 'primary'
         LEFT JOIN artists a ON a.id = ta.artist_id
         WHERE LOWER(t.title) = ? AND LOWER(COALESCE(a.name, t.artist, '')) = ?"
    )?;

    let track_ids: Vec<i64> = stmt
        .query_map([&normalized_title as &dyn rusqlite::ToSql,
            &normalized_artist as &dyn rusqlite::ToSql],
            |row| row.get::<_, i64>(0),
        )?
        .collect::<Result<_, _>>()?;
    drop(stmt);

    if track_ids.is_empty() {
        return Ok(0);
    }

    let tx = conn.transaction()?;
    let mut deleted = 0usize;
    for id in track_ids {
        deleted += tx.execute(
            "DELETE FROM canvas_url_cache WHERE track_id = ?",
            [id],
        )?;
    }
    tx.commit()?;
    Ok(deleted)
}
