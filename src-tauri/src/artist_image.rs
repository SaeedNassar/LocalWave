//! Faithful port of `server/src/services/artistImageService.ts`.
//! Searches Spotify for an artist image and caches it in SQLite.
//!
//! Rust-specific: DB connections from the r2d2 pool must NOT be held across
//! network `.await` points — the pool has only 5 slots and holding one during
//! a multi-second Spotify fetch would starve all other requests.

use std::collections::HashMap;

use reqwest::Method;
use serde_json::json;

use crate::db::{get_conn, DbPool};
use crate::spotify_api_helper::axios_retry_json;
use crate::spotify_auth::get_tokens;

const GRAPHQL_URL: &str = "https://api-partner.spotify.com/pathfinder/v2/query";
const SEARCH_HASH: &str = "fcad5a3e0d5af727fb76966f06971c19cfa2275e6ff7671196753e008611873c";
const CACHE_TTL_MS: i64 = 30 * 24 * 60 * 60 * 1000; // 30 days

#[derive(Debug, Clone)]
pub struct ArtistImageResult {
    pub image_url: Option<String>,
    pub spotify_url: Option<String>,
}

pub async fn get_artist_image(
    pool: &DbPool,
    artist_name: &str,
    artist_id: Option<i64>,
) -> ArtistImageResult {
    let lookup_name = artist_name.to_lowercase();

    // 1. Check DB cache (release connection before any await)
    let cached = {
        let conn = match get_conn(pool) {
            Ok(c) => c,
            Err(_) => return ArtistImageResult { image_url: None, spotify_url: None },
        };
        let row: Option<(Option<String>, Option<String>, String)> = conn
            .query_row(
                "SELECT image_url, spotify_url, fetched_at FROM artist_image_cache WHERE artist_name = ?",
                [&lookup_name],
                |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .ok();
        row
    };
    // conn dropped here — pool slot released

    let now = chrono::Utc::now();
    if let Some((image_url, spotify_url, fetched_at)) = cached {
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&fetched_at) {
            let age_ms = now.timestamp_millis() - dt.timestamp_millis();
            if age_ms < CACHE_TTL_MS {
                return ArtistImageResult { image_url, spotify_url };
            }
        }
    }

    // 2. Fetch tokens (network — no DB connection held)
    let (access_token, client_token) = match get_tokens(None).await {
        Ok(t) => t,
        Err(e) => {
            eprintln!("[artistImageService] Error: {}", e);
            return ArtistImageResult { image_url: None, spotify_url: None };
        }
    };

    // 3. Search Spotify (network — no DB connection held)
    match search_artist(artist_name, &access_token, &client_token).await {
        Ok(Some(result)) => {
            // 4. Write cache (new connection, quick write)
            if let Ok(conn) = get_conn(pool) {
                let fetched_at = now.to_rfc3339();
                let _ = conn.execute(
                    "INSERT OR REPLACE INTO artist_image_cache (artist_name, spotify_url, image_url, fetched_at) VALUES (?, ?, ?, ?)",
                    rusqlite::params![
                        &lookup_name,
                        result.spotify_url.as_deref(),
                        result.image_url.as_deref(),
                        fetched_at,
                    ],
                );

                if let (Some(img), Some(aid)) = (&result.image_url, artist_id) {
                    let _ = conn.execute(
                        "UPDATE artists SET image_path = ? WHERE id = ?",
                        rusqlite::params![img, aid],
                    );
                }
            }

            result
        }
        Ok(None) => {
            cache_empty(pool, &lookup_name);
            ArtistImageResult { image_url: None, spotify_url: None }
        }
        Err(e) => {
            eprintln!("[artistImageService] Error: {}", e);
            ArtistImageResult { image_url: None, spotify_url: None }
        }
    }
}

fn cache_empty(pool: &DbPool, lookup_name: &str) {
    if let Ok(conn) = get_conn(pool) {
        let now = chrono::Utc::now().to_rfc3339();
        let _ = conn.execute(
            "INSERT OR REPLACE INTO artist_image_cache (artist_name, spotify_url, image_url, fetched_at) VALUES (?, NULL, NULL, ?)",
            rusqlite::params![lookup_name, now],
        );
    }
}

async fn search_artist(
    artist_name: &str,
    access_token: &str,
    client_token: &str,
) -> Result<Option<ArtistImageResult>, String> {
    let mut names_to_try = vec![artist_name.to_string()];
    let re = regex::Regex::new(r"(?i)[,;/&]|\bfeat\.|\bft\.").map_err(|e| e.to_string())?;
    if let Some(first) = re.split(artist_name).next().map(|s| s.trim()) {
        if first != artist_name {
            names_to_try.push(first.to_string());
        }
    }

    let base_headers: HashMap<String, String> = [
        ("accept", "application/json"),
        ("content-type", "application/json;charset=UTF-8"),
        ("app-platform", "WebPlayer"),
        ("origin", "https://open.spotify.com"),
        ("referer", "https://open.spotify.com/"),
        ("spotify-app-version", "1.2.86.89.gf4a11fa1"),
        ("user-agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36"),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect();

    for name in names_to_try {
        let payload = json!({
            "operationName": "searchDesktop",
            "variables": {
                "searchTerm": name,
                "offset": 0,
                "limit": 10,
                "numberOfTopResults": 5,
                "includeAudiobooks": false,
                "includeArtistHasConcertsField": false,
                "includePreReleases": false,
                "includeAuthors": false,
            },
            "extensions": {
                "persistedQuery": { "version": 1, "sha256Hash": SEARCH_HASH },
            }
        });

        let mut headers = base_headers.clone();
        headers.insert("authorization".to_string(), format!("Bearer {}", access_token));
        headers.insert("client-token".to_string(), client_token.to_string());

        let res = axios_retry_json(
            GRAPHQL_URL,
            Method::POST,
            Some(&headers),
            Some(&payload),
            10000,
            3,
        )
        .await?;

        let items = res
            .data
            .get("data")
            .and_then(|v| v.get("searchV2"))
            .and_then(|v| v.get("artists"))
            .and_then(|v| v.get("items"))
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        if items.is_empty() {
            continue;
        }

        for item in items {
            let artist = item
                .get("data")
                .or_else(|| item.get("item").and_then(|v| v.get("data")))
                .cloned()
                .unwrap_or(serde_json::Value::Null);

            let uri = artist
                .get("uri")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if !uri.starts_with("spotify:artist:") {
                continue;
            }

            let spotify_name = artist
                .get("profile")
                .and_then(|v| v.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let lower_name = name.to_lowercase();
            let lower_spotify = spotify_name.to_lowercase();
            if !lower_spotify.contains(&lower_name) && !lower_name.contains(&lower_spotify) {
                continue;
            }

            let sources = artist
                .get("visuals")
                .and_then(|v| v.get("avatarImage"))
                .and_then(|v| v.get("sources"))
                .or_else(|| {
                    artist
                        .get("visuals")
                        .and_then(|v| v.get("headerImage"))
                        .and_then(|v| v.get("sources"))
                })
                .and_then(|v| v.as_array());

            let image_url = sources.and_then(|arr| {
                arr.first()
                    .and_then(|v| v.get("url"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            });

            if let Some(img) = image_url {
                let spotify_id = uri.split(':').last().unwrap_or("");
                let spotify_url = format!("https://open.spotify.com/artist/{}", spotify_id);
                return Ok(Some(ArtistImageResult {
                    image_url: Some(img),
                    spotify_url: Some(spotify_url),
                }));
            }
        }
    }

    Ok(None)
}
