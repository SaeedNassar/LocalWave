//! Playlist import from .m3u/.m3u8 files. Port of
//! `server/src/services/importPlaylist.ts`.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use chrono::Utc;
use rusqlite::{params, Connection};

use crate::m3u_parser::read_m3u_file;

#[derive(Debug, Clone, Default)]
pub struct ImportPlaylistResult {
    pub playlist_id: i64,
    pub total_entries: i64,
    pub matched: i64,
    pub missing: i64,
    pub warnings: Vec<String>,
}

struct TrackRow {
    id: i64,
    path: String,
    filename: String,
}

pub fn import_playlist_from_file(
    conn: &mut Connection,
    file_path: &Path,
    force: bool,
) -> Result<ImportPlaylistResult, String> {
    let abs = std::fs::canonicalize(file_path)
        .unwrap_or_else(|_| file_path.to_path_buf());
    if !abs.is_file() {
        return Err(format!("File not found: {}", abs.display()));
    }
    let ext = abs
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    if ext != "m3u" && ext != "m3u8" {
        return Err("Only .m3u/.m3u8 files are supported".into());
    }

    // skip already-imported unless forced
    if !force {
        let existing = conn
            .query_row(
                "SELECT id FROM playlists WHERE source = ? AND is_imported = 1",
                [abs.to_string_lossy().as_ref()],
                |row| row.get::<usize, i64>(0),
            )
            .ok();
        if existing.is_some() {
            return Ok(ImportPlaylistResult::default());
        }
    }

    let parsed = read_m3u_file(&abs).map_err(|e| e.to_string())?;

    // build track lookup maps
    let mut stmt = conn
        .prepare("SELECT id, path, filename FROM tracks")
        .map_err(|e| e.to_string())?;
    let all_tracks: Vec<TrackRow> = stmt
        .query_map([], |row| {
            Ok(TrackRow {
                id: row.get(0)?,
                path: row.get::<_, String>(1)?.to_lowercase(),
                filename: row.get::<_, String>(2)?.to_lowercase(),
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    drop(stmt);

    let mut by_filename: HashMap<String, i64> = HashMap::new();
    let mut by_path: HashMap<String, i64> = HashMap::new();
    for t in &all_tracks {
        by_filename.insert(t.filename.clone(), t.id);
        by_path.insert(t.path.clone(), t.id);
    }

    let match_entry = |entry_path: Option<&PathBuf>| -> Option<i64> {
        let p = entry_path?;
        let lower = p.to_string_lossy().to_lowercase();
        if let Some(id) = by_path.get(&lower) {
            return Some(*id);
        }
        let fname = p
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        by_filename.get(&fname).copied()
    };

    let playlist_name = parsed
        .name
        .clone()
        .unwrap_or_else(|| abs.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default());

    let existing_id = conn
        .query_row(
            "SELECT id FROM playlists WHERE source = ? AND is_imported = 1",
            [abs.to_string_lossy().as_ref()],
            |row| row.get::<usize, i64>(0),
        )
        .ok();

    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let playlist_id = if force && existing_id.is_some() {
        let id = existing_id.unwrap();
        tx.execute("UPDATE playlists SET name = ? WHERE id = ?", params![&playlist_name, id])
            .map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM playlist_entries WHERE playlist_id = ?", [id])
            .map_err(|e| e.to_string())?;
        id
    } else {
        let max_pos: i64 = tx
            .query_row("SELECT COALESCE(MAX(position), -1) FROM playlists", [], |row| row.get(0))
            .unwrap_or(-1);
        tx.execute(
            "INSERT INTO playlists (name, description, created_at, is_imported, source, position) VALUES (?, ?, ?, 1, ?, ?)",
            params![&playlist_name, Option::<String>::None, Utc::now().to_rfc3339(), abs.to_string_lossy().as_ref(), max_pos + 1],
        )
        .map_err(|e| e.to_string())?;
        tx.last_insert_rowid()
    };

    let mut position = 0i64;
    for e in &parsed.entries {
        let matched = match_entry(e.found_path.as_ref());
        tx.execute(
            "INSERT INTO playlist_entries (playlist_id, position, track_id, raw_entry, missing, title) VALUES (?, ?, ?, ?, ?, ?)",
            params![
                playlist_id,
                position,
                matched,
                &e.raw_entry,
                if matched.is_some() { 0 } else { 1 },
                &e.title,
            ],
        )
        .map_err(|e| e.to_string())?;
        position += 1;
    }
    tx.commit().map_err(|e| e.to_string())?;

    let missing_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM playlist_entries WHERE playlist_id = ? AND missing = 1",
            [playlist_id],
            |row| row.get(0),
        )
        .unwrap_or(0);
    let total: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM playlist_entries WHERE playlist_id = ?",
            [playlist_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    Ok(ImportPlaylistResult {
        playlist_id,
        total_entries: total,
        matched: total - missing_count,
        missing: missing_count,
        warnings: parsed.warnings,
    })
}

pub fn remove_stale_imported_playlists(
    conn: &Connection,
    known_sources: &HashSet<String>,
) -> Result<(), String> {
    let mut stmt = conn
        .prepare("SELECT id, source FROM playlists WHERE is_imported = 1")
        .map_err(|e| e.to_string())?;
    let rows: Vec<(i64, Option<String>)> = stmt
        .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?)))
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    drop(stmt);

    for (id, source) in rows {
        if let Some(src) = source {
            if !known_sources.contains(&src) {
                let _ = conn.execute("DELETE FROM playlists WHERE id = ?", [id]);
            }
        }
    }
    Ok(())
}
