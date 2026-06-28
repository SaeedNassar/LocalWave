//! Runtime configuration (config.json) and data-dir resolution.
//!
//! In the original Node app, data lived in `server/data/`. For the packaged
//! Tauri app we use the per-user app-data directory (`%APPDATA%/LocalWave`),
//! which survives updates and is writable without elevation.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use once_cell::sync::Lazy;
use serde_json::Value;

use crate::types::AppConfig;

static DATA_DIR: Lazy<PathBuf> = Lazy::new(|| {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("LocalWave")
});

static CONFIG_CACHE: Lazy<Mutex<Option<AppConfig>>> = Lazy::new(|| Mutex::new(None));
static WRITE_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

pub fn data_dir() -> &'static Path {
    &DATA_DIR
}

pub fn db_path() -> PathBuf {
    DATA_DIR.join("localwave.db")
}

fn config_path() -> PathBuf {
    DATA_DIR.join("config.json")
}

fn default_config() -> AppConfig {
    let music_folder = dirs::audio_dir()
        .or_else(dirs::home_dir)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "C:\\Users\\Public\\Music".to_string());

    AppConfig {
        music_folder,
        supported_extensions: vec![
            ".mp3".into(),
            ".flac".into(),
            ".wav".into(),
            ".m4a".into(),
        ],
        scan_interval_ms: 0,
        port: 8787,
        sp_dc: String::new(),
        enable_lyrics: true,
        enable_canvas: false,
        musixmatch_access_token: String::new(),
    }
}

pub fn ensure_data_dir() -> std::io::Result<()> {
    fs::create_dir_all(&*DATA_DIR)?;
    Ok(())
}

/// Merge a partial JSON patch onto the defaults. Unknown keys are ignored,
/// missing keys keep their current value.
fn merge_json(base: &Value, patch: &Value) -> Value {
    match (base, patch) {
        (Value::Object(base_map), Value::Object(patch_map)) => {
            let mut out = base_map.clone();
            for (k, v) in patch_map {
                if v.is_null() {
                    continue;
                }
                out.insert(k.clone(), v.clone());
            }
            Value::Object(out)
        }
        (_, patch_val) => patch_val.clone(),
    }
}

pub fn load_config() -> std::io::Result<AppConfig> {
    {
        let guard = CONFIG_CACHE.lock().unwrap();
        if let Some(c) = guard.as_ref() {
            return Ok(c.clone());
        }
    }

    ensure_data_dir()?;
    let path = config_path();
    let cfg = match fs::read_to_string(&path) {
        Ok(raw) => {
            let defaults = serde_json::to_value(default_config()).unwrap();
            let parsed: Value = serde_json::from_str(&raw).unwrap_or_else(|e| {
                // corrupt config — back it up so secrets aren't clobbered.
                let _ = fs::copy(&path, format!("{}.broken", path.to_string_lossy()));
                log::warn!("config.json was corrupt ({e}) — backed up to .broken");
                defaults.clone()
            });
            let merged = merge_json(&defaults, &parsed);
            serde_json::from_value(merged).unwrap_or_else(|_| default_config())
        }
        Err(_) => {
            let c = default_config();
            let _ = save_config(&c);
            c
        }
    };

    let mut guard = CONFIG_CACHE.lock().unwrap();
    *guard = Some(cfg.clone());
    Ok(cfg)
}

pub fn save_config(cfg: &AppConfig) -> std::io::Result<()> {
    let _wl = WRITE_LOCK.lock().unwrap();
    ensure_data_dir()?;
    let tmp = config_path().with_extension("json.tmp");
    let json = serde_json::to_string_pretty(cfg).unwrap();
    fs::write(&tmp, json)?;
    fs::rename(&tmp, config_path())?;
    let mut guard = CONFIG_CACHE.lock().unwrap();
    *guard = Some(cfg.clone());
    Ok(())
}

/// Apply a partial JSON patch (camelCase keys) to the cached config and persist.
/// Returns the new full config. Validates a few critical fields.
pub fn patch_config(patch: &Value) -> Result<AppConfig, String> {
    if let Some(mf) = patch.get("musicFolder") {
        if !mf.is_null() && !mf.is_string() {
            return Err("musicFolder must be a string".into());
        }
        if let Some(s) = mf.as_str() {
            let meta = fs::metadata(s).map_err(|_| "Folder does not exist".to_string())?;
            if !meta.is_dir() {
                return Err("Folder does not exist or is not a directory".into());
            }
        }
    }
    if let Some(p) = patch.get("port") {
        if let Some(n) = p.as_u64() {
            if !(1..=65535).contains(&n) {
                return Err("port must be an integer in 1..65535".into());
            }
        } else if !p.is_null() {
            return Err("port must be an integer in 1..65535".into());
        }
    }

    let current = load_config().map_err(|e| e.to_string())?;
    let base = serde_json::to_value(&current).unwrap();
    let merged = merge_json(&base, patch);
    let new_cfg: AppConfig =
        serde_json::from_value(merged).map_err(|e| format!("invalid config: {e}"))?;
    save_config(&new_cfg).map_err(|e| e.to_string())?;
    Ok(new_cfg)
}
