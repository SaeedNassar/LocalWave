//! Faithful port of `server/src/services/spotifyAuthService.ts`.
//! Includes TOTP initialization from GitHub secrets or fallback, access/client
//! token fetching, caching, expiry, and automatic periodic refresh.

use std::time::{SystemTime, UNIX_EPOCH};

use hmac::{Hmac, Mac};
use reqwest::Method;
use serde_json::json;
use sha1::Sha1;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::spotify_api_helper::axios_retry_json;

type HmacSha1 = Hmac<Sha1>;

// ── Token cache ─────────────────────────────────────────────

#[derive(Clone, Debug)]
struct CachedToken {
    token: String,
    expires_at: i64,
}

static ACCESS_TOKEN_CACHE: Mutex<Option<CachedToken>> = Mutex::const_new(None);
static CLIENT_TOKEN_CACHE: Mutex<Option<CachedToken>> = Mutex::const_new(None);

// Access/client fetches are single-flight: concurrent callers wait on this
// mutex and then benefit from the shared cache populated by the first caller.
static ACCESS_TOKEN_LOCK: Mutex<()> = Mutex::const_new(());
static CLIENT_TOKEN_LOCK: Mutex<()> = Mutex::const_new(());

// ── TOTP secrets ────────────────────────────────────────────

const TOTP_SECRETS_URL: &str =
    "https://raw.githubusercontent.com/xyloflake/spot-secrets-go/refs/heads/main/secrets/secretDict.json";
const FETCH_INTERVAL_MS: i64 = 3600_000; // 1 hour

// Fallback secret — used when GitHub secrets can't be fetched
const FALLBACK_TOTP_SECRET: [u8; 26] = [
    99, 111, 47, 88, 49, 56, 118, 65, 52, 67, 50, 104, 117, 101, 55, 94, 95, 75, 94, 49, 69, 36, 85, 64, 74, 60,
];

#[derive(Clone, Debug)]
struct TotpState {
    secret_bytes: Vec<u8>,
    version: String,
}

static CURRENT_TOTP: Mutex<Option<TotpState>> = Mutex::const_new(None);
static LAST_TOTP_FETCH_MS: Mutex<i64> = Mutex::const_new(0);

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

pub async fn refresh_totp_secrets() {
    if let Err(e) = update_totp_secrets().await {
        eprintln!("[spotify] Failed to refresh TOTP secrets: {}", e);
        let has_totp = CURRENT_TOTP.lock().await.is_some();
        if !has_totp {
            use_fallback_secret().await;
        }
    }
}

async fn initialize_totp_secrets() {
    if let Err(e) = update_totp_secrets().await {
        eprintln!("[spotify] Failed to initialize TOTP secrets: {}", e);
    }
    let has_totp = CURRENT_TOTP.lock().await.is_some();
    if !has_totp {
        use_fallback_secret().await;
    }
}

async fn update_totp_secrets() -> Result<(), String> {
    let now = now_ms();
    {
        let last = LAST_TOTP_FETCH_MS.lock().await;
        if now - *last < FETCH_INTERVAL_MS {
            return Ok(());
        }
    }

    let secrets: serde_json::Value = fetch_secrets_from_github().await?;
    let newest_version = find_newest_version(&secrets);

    if let Some(version) = newest_version {
        let current_version = CURRENT_TOTP.lock().await.as_ref().map(|s| s.version.clone());
        if current_version.as_ref() != Some(&version) {
            let secret_bytes = create_totp_secret(&secrets, &version)?;
            *CURRENT_TOTP.lock().await = Some(TotpState {
                secret_bytes,
                version: version.clone(),
            });
            *LAST_TOTP_FETCH_MS.lock().await = now;
            println!("[spotify] TOTP secrets updated to version {}", version);
        } else {
            println!(
                "[spotify] No new TOTP secrets found, using version {}",
                current_version.unwrap_or_else(|| "fallback".to_string())
            );
        }
    } else {
        println!(
            "[spotify] No new TOTP secrets found, using version {}",
            CURRENT_TOTP.lock().await.as_ref().map(|s| s.version.clone()).unwrap_or_else(|| "fallback".to_string())
        );
    }

    Ok(())
}

async fn fetch_secrets_from_github() -> Result<serde_json::Value, String> {
    let mut headers = std::collections::HashMap::new();
    headers.insert(
        "User-Agent".to_string(),
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36".to_string(),
    );

    let res = axios_retry_json(
        TOTP_SECRETS_URL,
        Method::GET,
        Some(&headers),
        None,
        10000,
        3,
    )
    .await?;

    Ok(res.data)
}

fn find_newest_version(secrets: &serde_json::Value) -> Option<String> {
    secrets
        .as_object()
        .map(|obj| {
            obj.keys()
                .filter_map(|k| k.parse::<i64>().ok())
                .max()
                .map(|m| m.to_string())
        })
        .unwrap_or(None)
}

fn create_totp_secret(secrets: &serde_json::Value, version: &str) -> Result<Vec<u8>, String> {
    let data = secrets
        .get(version)
        .and_then(|v| v.as_array())
        .ok_or_else(|| format!("No secret data for version {}", version))?;

    let mapped: Vec<i64> = data
        .iter()
        .enumerate()
        .map(|(index, val)| {
            let value = val.as_i64().unwrap_or(0);
            value ^ ((index as i64 % 33) + 9)
        })
        .collect();

    let joined = mapped
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join("");

    Ok(joined.into_bytes())
}

async fn use_fallback_secret() {
    let secret_bytes = create_totp_secret_from_slice(&FALLBACK_TOTP_SECRET);
    *CURRENT_TOTP.lock().await = Some(TotpState {
        secret_bytes,
        version: "19".to_string(),
    });
    println!("[spotify] Using fallback TOTP secret");
}

fn create_totp_secret_from_slice(data: &[u8]) -> Vec<u8> {
    let mapped: Vec<i64> = data
        .iter()
        .enumerate()
        .map(|(index, &value)| {
            let value = value as i64;
            value ^ ((index as i64 % 33) + 9)
        })
        .collect();

    mapped
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join("")
        .into_bytes()
}

async fn generate_totp(timestamp_ms: i64) -> Result<String, String> {
    let state = {
        let guard = CURRENT_TOTP.lock().await;
        guard.as_ref().cloned().ok_or_else(|| "TOTP not initialized".to_string())?
    };
    // Compute TOTP per RFC 4226 / RFC 6238: counter = timestamp_ms / 1000 / 30.
    let counter = (timestamp_ms / 1000 / 30) as u64;
    let counter_bytes = counter.to_be_bytes();

    let mut mac = HmacSha1::new_from_slice(&state.secret_bytes)
        .map_err(|e| format!("HMAC init failed: {}", e))?;
    mac.update(&counter_bytes);
    let result = mac.finalize();
    let hash = result.into_bytes();

    let offset = (hash[hash.len() - 1] & 0x0f) as usize;
    let code = (((hash[offset] & 0x7f) as u32) << 24)
        | (((hash[offset + 1] & 0xff) as u32) << 16)
        | (((hash[offset + 2] & 0xff) as u32) << 8)
        | ((hash[offset + 3] & 0xff) as u32);

    let code = (code % 1_000_000) as u32;
    Ok(format!("{:06}", code))
}

fn user_agent() -> String {
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/127.0.0.0 Safari/537.36".to_string()
}

// ── Server time ─────────────────────────────────────────────

async fn get_server_time(sp_dc: Option<&str>) -> i64 {
    let mut headers = std::collections::HashMap::new();
    headers.insert("Origin".to_string(), "https://open.spotify.com/".to_string());
    headers.insert("Referer".to_string(), "https://open.spotify.com/".to_string());
    headers.insert("User-Agent".to_string(), user_agent());
    if let Some(dc) = sp_dc {
        headers.insert("Cookie".to_string(), format!("sp_dc={}", dc));
    }

    match axios_retry_json(
        "https://open.spotify.com/api/server-time",
        Method::GET,
        Some(&headers),
        None,
        5000,
        3,
    )
    .await
    {
        Ok(res) => {
            let server_time = res.data.get("serverTime").and_then(|v| v.as_i64()).unwrap_or(0);
            if server_time == 0 {
                now_ms()
            } else {
                server_time * 1000
            }
        }
        Err(_) => now_ms(),
    }
}

// ── Access token ────────────────────────────────────────────

pub async fn get_access_token(sp_dc: Option<&str>) -> Result<String, String> {
    {
        let cache = ACCESS_TOKEN_CACHE.lock().await;
        if let Some(c) = cache.as_ref() {
            if c.expires_at > now_ms() {
                let remaining_secs = (c.expires_at - now_ms()) / 1000;
                log::info!("[spotify-auth] Access token cache HIT — expires in {}s", remaining_secs);
                return Ok(c.token.clone());
            } else {
                log::info!("[spotify-auth] Access token cache EXPIRED — refetching...");
            }
        } else {
            log::info!("[spotify-auth] Access token cache EMPTY — fetching new token...");
        }
    }

    let _guard = ACCESS_TOKEN_LOCK.lock().await;

    // Double-check after acquiring single-flight lock
    {
        let cache = ACCESS_TOKEN_CACHE.lock().await;
        if let Some(c) = cache.as_ref() {
            if c.expires_at > now_ms() {
                log::info!("[spotify-auth] Access token cache HIT (post-lock) — another request already refreshed it");
                return Ok(c.token.clone());
            }
        }
    }

    fetch_access_token(sp_dc).await
}

async fn fetch_access_token(sp_dc: Option<&str>) -> Result<String, String> {
    let has_totp = CURRENT_TOTP.lock().await.is_some();
    if !has_totp {
        initialize_totp_secrets().await;
    }

    let server_time = get_server_time(sp_dc).await;
    let local_time = now_ms();

    let totp_local = generate_totp(local_time).await?;
    let totp_server = generate_totp(server_time).await?;
    let totp_version = CURRENT_TOTP
        .lock()
        .await
        .as_ref()
        .map(|s| s.version.clone())
        .unwrap_or_else(|| "19".to_string());

    let product_type = if sp_dc.is_some() { "web_player" } else { "mobile-web-player" };
    let url = format!(
        "https://open.spotify.com/api/token?reason=transport&productType={}&totp={}&totpServer={}&totpVer={}",
        urlencoding::encode(product_type),
        totp_local,
        totp_server,
        totp_version
    );

    let mut headers = std::collections::HashMap::new();
    headers.insert("Origin".to_string(), "https://open.spotify.com/".to_string());
    headers.insert("Referer".to_string(), "https://open.spotify.com/".to_string());
    headers.insert("User-Agent".to_string(), user_agent());
    if let Some(dc) = sp_dc {
        headers.insert("Cookie".to_string(), format!("sp_dc={}", dc));
    }

    let res = axios_retry_json(
        &url,
        Method::GET,
        Some(&headers),
        None,
        10000,
        3,
    )
    .await?;

    let access_token = res
        .data
        .get("accessToken")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing accessToken in Spotify response".to_string())?
        .to_string();
    let expires_at = res
        .data
        .get("accessTokenExpirationTimestampMs")
        .and_then(|v| v.as_i64())
        .unwrap_or_else(|| now_ms() + 3600_000);
    let client_id = res
        .data
        .get("clientId")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let ttl_secs = (expires_at - now_ms()) / 1000;
    log::info!(
        "[spotify-auth] Access token fetched OK — length={}, expires in {}s (at {})",
        access_token.len(),
        ttl_secs,
        expires_at
    );
    if ttl_secs < 300 {
        log::warn!("[spotify-auth] Access token TTL is very short (<5min) — may expire soon");
    }

    *ACCESS_TOKEN_CACHE.lock().await = Some(CachedToken {
        token: access_token.clone(),
        expires_at,
    });

    // Pre-fetch client token if we have a clientId
    if let Some(cid) = client_id {
        let client_cache = CLIENT_TOKEN_CACHE.lock().await;
        if client_cache.as_ref().map(|c| c.expires_at).unwrap_or(0) < now_ms() {
            log::info!("[spotify-auth] Client token expired or missing — pre-fetching with clientId={}", cid);
            drop(client_cache);
            tokio::spawn(async move {
                let _ = fetch_client_token(Some(&cid)).await;
            });
        }
    }

    Ok(access_token)
}

// ── Client token ────────────────────────────────────────────

pub async fn get_client_token(sp_dc: Option<&str>) -> Result<String, String> {
    {
        let cache = CLIENT_TOKEN_CACHE.lock().await;
        if let Some(c) = cache.as_ref() {
            if c.expires_at > now_ms() {
                let remaining_secs = (c.expires_at - now_ms()) / 1000;
                log::info!("[spotify-auth] Client token cache HIT — expires in {}s", remaining_secs);
                return Ok(c.token.clone());
            } else {
                log::info!("[spotify-auth] Client token cache EXPIRED — refetching...");
            }
        } else {
            log::info!("[spotify-auth] Client token cache EMPTY — fetching new token...");
        }
    }

    let _guard = CLIENT_TOKEN_LOCK.lock().await;

    {
        let cache = CLIENT_TOKEN_CACHE.lock().await;
        if let Some(c) = cache.as_ref() {
            if c.expires_at > now_ms() {
                log::info!("[spotify-auth] Client token cache HIT (post-lock) — another request already refreshed it");
                return Ok(c.token.clone());
            }
        }
    }

    // We need a clientId — get it from the access token flow if missing
    {
        let access_cache = ACCESS_TOKEN_CACHE.lock().await;
        if access_cache.is_none() {
            drop(access_cache);
            get_access_token(sp_dc).await?;
        }
    }

    fetch_client_token(None).await
}

async fn fetch_client_token(client_id: Option<&str>) -> Result<String, String> {
    let client_id = if let Some(id) = client_id {
        id.to_string()
    } else {
        // Re-fetch access token to extract clientId
        let _server_time = get_server_time(None).await;
        let totp = generate_totp(now_ms()).await?;
        let totp_version = CURRENT_TOTP
            .lock()
            .await
            .as_ref()
            .map(|s| s.version.clone())
            .unwrap_or_else(|| "19".to_string());

        let url = format!(
            "https://open.spotify.com/api/token?reason=transport&productType=web_player&totp={}&totpVer={}",
            totp, totp_version
        );

        let mut headers = std::collections::HashMap::new();
        headers.insert("Origin".to_string(), "https://open.spotify.com/".to_string());
        headers.insert("Referer".to_string(), "https://open.spotify.com/".to_string());
        headers.insert("User-Agent".to_string(), user_agent());

        let res = axios_retry_json(
            &url,
            Method::GET,
            Some(&headers),
            None,
            10000,
            3,
        )
        .await?;

        res.data
            .get("clientId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "No clientId for client token".to_string())?
            .to_string()
    };

    let body = json!({
        "client_data": {
            "client_version": "1.2.42.1100",
            "client_id": client_id,
            "js_sdk_data": {
                "device_brand": "unknown",
                "device_model": "web_player",
                "os": "windows",
                "os_version": "10",
                "container_version": "1.2.42",
                "device_id": Uuid::new_v4().to_string(),
                "device_type": "computer",
                "platform_identifier": "web_player",
            },
        }
    });

    let mut headers = std::collections::HashMap::new();
    headers.insert("Content-Type".to_string(), "application/json".to_string());
    headers.insert("Accept".to_string(), "application/json".to_string());
    headers.insert("User-Agent".to_string(), user_agent());

    let res = axios_retry_json(
        "https://clienttoken.spotify.com/v1/clienttoken",
        Method::POST,
        Some(&headers),
        Some(&body),
        10000,
        3,
    )
    .await?;

    let granted = res
        .data
        .get("granted_token")
        .ok_or_else(|| "Missing granted_token".to_string())?;
    let token = granted
        .get("token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing granted_token.token".to_string())?
        .to_string();
    let refresh_after = granted
        .get("refresh_after_seconds")
        .and_then(|v| v.as_i64())
        .unwrap_or(3600);

    *CLIENT_TOKEN_CACHE.lock().await = Some(CachedToken {
        token: token.clone(),
        expires_at: now_ms() + refresh_after * 1000,
    });

    log::info!(
        "[spotify-auth] Client token fetched OK — length={}, refresh_after={}s (expires in {}s)",
        token.len(),
        refresh_after,
        refresh_after
    );

    Ok(token)
}

pub async fn get_tokens(sp_dc: Option<&str>) -> Result<(String, String), String> {
    let (access, client) = tokio::join!(
        get_access_token(sp_dc),
        get_client_token(sp_dc)
    );
    Ok((access?, client?))
}

pub async fn invalidate_tokens() {
    *ACCESS_TOKEN_CACHE.lock().await = None;
    *CLIENT_TOKEN_CACHE.lock().await = None;
}

// ── Init: refresh TOTP secrets on startup and periodically ──

pub async fn init_spotify_auth() {
    initialize_totp_secrets().await;
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(FETCH_INTERVAL_MS as u64)).await;
            refresh_totp_secrets().await;
        }
    });
}
