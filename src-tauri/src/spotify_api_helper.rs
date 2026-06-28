//! Faithful port of `server/src/services/spotifyApiHelper.ts`.
//! Thin reqwest wrapper with retry-on-429, respecting Retry-After header.

use std::collections::HashMap;
use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderName, HeaderValue, RETRY_AFTER};
use reqwest::Method;
use serde::de::DeserializeOwned;

pub struct RetryResult<T> {
    pub data: T,
    pub status: u16,
    pub headers: HashMap<String, String>,
}

/// Error type that preserves whether a failure was a 429 (rate limited)
/// vs. anything else, so callers can decide not to cache-negative on 429.
#[derive(Debug, Clone)]
pub enum ApiError {
    /// Exhausted all retries while still receiving 429s.
    RateLimited { retry_after_ms: Option<u64> },
    /// Any other failure: network error, non-429 HTTP status, parse error, etc.
    Other(String),
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError::RateLimited { retry_after_ms } => {
                write!(f, "rate limited (retry_after_ms={:?})", retry_after_ms)
            }
            ApiError::Other(s) => write!(f, "{}", s),
        }
    }
}

impl ApiError {
    pub fn is_rate_limited(&self) -> bool {
        matches!(self, ApiError::RateLimited { .. })
    }
}

impl From<ApiError> for String {
    fn from(e: ApiError) -> String {
        e.to_string()
    }
}

fn serialize_body(data: &serde_json::Value) -> Option<Vec<u8>> {
    if data.is_null() {
        None
    } else {
        Some(serde_json::to_vec(data).unwrap_or_default())
    }
}

/// Parses Retry-After header value (seconds, per RFC) into milliseconds.
/// Falls back to exponential backoff if the header is missing or unparseable.
fn retry_after_ms(res: &reqwest::Response, attempt: u32) -> u64 {
    res.headers()
        .get(RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .map(|secs| secs * 1000)
        .unwrap_or_else(|| (2_u64.pow(attempt + 1)) * 1000)
}

pub async fn axios_retry_json(
    url: &str,
    method: Method,
    headers: Option<&HashMap<String, String>>,
    data: Option<&serde_json::Value>,
    timeout_ms: u64,
    retries: u32,
) -> Result<RetryResult<serde_json::Value>, ApiError> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(timeout_ms.max(100)))
        .build()
        .map_err(|e| ApiError::Other(e.to_string()))?;

    let mut last_error = ApiError::Other("Request failed after retries".to_string());

    for attempt in 0..=retries {
        let mut req = client.request(method.clone(), url);
        if let Some(h) = headers {
            let mut hm = HeaderMap::new();
            for (k, v) in h {
                if let (Ok(name), Ok(val)) = (
                    HeaderName::from_bytes(k.as_bytes()),
                    HeaderValue::from_str(v),
                ) {
                    hm.insert(name, val);
                }
            }
            req = req.headers(hm);
        }
        if let Some(d) = data {
            if let Some(body) = serialize_body(d) {
                req = req.body(body);
            }
        }

        let res = match req.send().await {
            Ok(r) => r,
            Err(e) => {
                last_error = ApiError::Other(e.to_string());
                if attempt < retries {
                    tokio::time::sleep(Duration::from_millis(
                        (2_u64.pow(attempt + 1)) * 1000,
                    ))
                    .await;
                    continue;
                }
                return Err(last_error);
            }
        };

        let status = res.status().as_u16();

        if status == 429 {
            let delay_ms = retry_after_ms(&res, attempt);
            last_error = ApiError::RateLimited {
                retry_after_ms: Some(delay_ms),
            };
            if attempt < retries {
                log::warn!(
                    "[api-helper] 429 on {} (attempt {}/{}) — waiting {}ms",
                    url, attempt + 1, retries + 1, delay_ms
                );
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                continue;
            }
            // Exhausted retries while still rate limited — return typed error.
            return Err(last_error);
        }

        let resp_headers: HashMap<String, String> = res
            .headers()
            .iter()
            .filter_map(|(k, v)| v.to_str().ok().map(|s| (k.to_string(), s.to_string())))
            .collect();

        let text = res.text().await.map_err(|e| ApiError::Other(e.to_string()))?;
        let json: serde_json::Value =
            serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);

        return Ok(RetryResult {
            data: json,
            status,
            headers: resp_headers,
        });
    }

    Err(last_error)
}

pub async fn axios_retry_bytes(
    url: &str,
    method: Method,
    headers: Option<&HashMap<String, String>>,
    data: Option<Vec<u8>>,
    timeout_ms: u64,
    retries: u32,
) -> Result<RetryResult<Vec<u8>>, ApiError> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(timeout_ms.max(100)))
        .build()
        .map_err(|e| ApiError::Other(e.to_string()))?;

    let mut last_error = ApiError::Other("Request failed after retries".to_string());

    for attempt in 0..=retries {
        let mut req = client.request(method.clone(), url);
        if let Some(h) = headers {
            let mut hm = HeaderMap::new();
            for (k, v) in h {
                if let (Ok(name), Ok(val)) = (
                    HeaderName::from_bytes(k.as_bytes()),
                    HeaderValue::from_str(v),
                ) {
                    hm.insert(name, val);
                }
            }
            req = req.headers(hm);
        }
        if let Some(b) = &data {
            req = req.body(b.clone());
        }

        let res = match req.send().await {
            Ok(r) => r,
            Err(e) => {
                last_error = ApiError::Other(e.to_string());
                if attempt < retries {
                    tokio::time::sleep(Duration::from_millis(
                        (2_u64.pow(attempt + 1)) * 1000,
                    ))
                    .await;
                    continue;
                }
                return Err(last_error);
            }
        };

        let status = res.status().as_u16();

        if status == 429 {
            let delay_ms = retry_after_ms(&res, attempt);
            last_error = ApiError::RateLimited {
                retry_after_ms: Some(delay_ms),
            };
            if attempt < retries {
                log::warn!(
                    "[api-helper] 429 on {} (attempt {}/{}) — waiting {}ms",
                    url, attempt + 1, retries + 1, delay_ms
                );
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                continue;
            }
            return Err(last_error);
        }

        let resp_headers: HashMap<String, String> = res
            .headers()
            .iter()
            .filter_map(|(k, v)| v.to_str().ok().map(|s| (k.to_string(), s.to_string())))
            .collect();

        let bytes = res.bytes().await.map_err(|e| ApiError::Other(e.to_string()))?.to_vec();

        return Ok(RetryResult {
            data: bytes,
            status,
            headers: resp_headers,
        });
    }

    Err(last_error)
}

/// Fetch and parse JSON with typed deserialization (used by musixmatch).
pub async fn fetch_json<T: DeserializeOwned>(
    url: &str,
    timeout_ms: u64,
) -> Result<Option<T>, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(timeout_ms))
        .build()
        .map_err(|e| e.to_string())?;
    let res = client
        .get(url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !res.status().is_success() {
        return Ok(None);
    }
    let body: T = res.json().await.map_err(|e| e.to_string())?;
    Ok(Some(body))
}