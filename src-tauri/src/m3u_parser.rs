//! Faithful port of `server/src/services/m3u8Parser.ts`.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct M3uEntry {
    pub raw_entry: String,
    pub title: Option<String>,
    pub found_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Default)]
pub struct M3uParseResult {
    pub entries: Vec<M3uEntry>,
    pub name: Option<String>,
    pub warnings: Vec<String>,
}

pub fn parse_m3u(content: &str, base_dir: Option<&Path>) -> M3uParseResult {
    let mut entries = Vec::new();
    let mut warnings = Vec::new();
    let mut pending_title: Option<String> = None;
    let mut name: Option<String> = None;

    for raw in content.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with("#EXTM3U") {
            continue;
        }
        if let Some(rest) = line.strip_prefix("#PLAYLIST:") {
            name = Some(rest.trim().to_string());
            continue;
        }
        if let Some(rest) = line.strip_prefix("#EXTINF:") {
            pending_title = match rest.find(',') {
                Some(idx) => Some(rest[idx + 1..].trim().to_string()),
                None => None,
            };
            continue;
        }
        if line.starts_with("#EXTGRP:") || line.starts_with("#EXTALB:") || line.starts_with("#EXTART:") {
            continue;
        }
        if line.starts_with('#') {
            continue;
        }

        let resolved = resolve_entry_path(line, base_dir);
        if resolved.is_none() {
            warnings.push(format!("Could not resolve or find entry: {line}"));
        }
        entries.push(M3uEntry {
            raw_entry: line.to_string(),
            title: pending_title.take(),
            found_path: resolved,
        });
    }

    M3uParseResult { entries, name, warnings }
}

fn resolve_entry_path(entry: &str, base_dir: Option<&Path>) -> Option<PathBuf> {
    // URI / stream
    let looks_like_uri = {
        let bytes = entry.as_bytes();
        let mut i = 0;
        while i < bytes.len() && bytes[i] != b':' {
            if !bytes[i].is_ascii_alphabetic() {
                break;
            }
            i += 1;
        }
        i > 0 && i + 2 <= bytes.len() && bytes[i] == b':' && bytes.get(i + 1) == Some(&b'/') && bytes.get(i + 2) == Some(&b'/')
    };
    if looks_like_uri {
        return None;
    }

    let candidate = if Path::new(entry).is_absolute() {
        PathBuf::from(entry)
    } else if let Some(base) = base_dir {
        base.join(entry)
    } else {
        PathBuf::from(".").join(entry)
    };

    match std::fs::metadata(&candidate) {
        Ok(meta) if meta.is_file() => Some(candidate),
        _ => None,
    }
}

pub fn read_m3u_file(file_path: &Path) -> std::io::Result<M3uParseResult> {
    let content = std::fs::read_to_string(file_path)?;
    let base_dir = file_path.parent();
    let mut result = parse_m3u(&content, base_dir);
    if result.name.is_none() {
        if let Some(stem) = file_path.file_stem() {
            result.name = Some(stem.to_string_lossy().into_owned());
        }
    }
    Ok(result)
}
