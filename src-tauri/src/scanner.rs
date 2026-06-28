//! Library scanner. Faithful port of `server/src/services/scanner.ts`.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::Utc;
use once_cell::sync::Lazy;
use rusqlite::{params, Connection};
use walkdir::WalkDir;

use crate::artist_parser::{normalize_artist_name, split_artists};
use crate::db::{attach_track_artists, get_conn, row_to_track, DbPool};
use crate::metadata::parse_track_metadata;
use crate::types::{ScanProgress, Track};

pub const PLAYLIST_EXTS: Lazy<HashSet<String>> =
    Lazy::new(|| [".m3u".to_string(), ".m3u8".to_string()].into_iter().collect());

fn is_supported_audio(ext: &str, supported: &[Arc<str>]) -> bool {
    supported.iter().any(|e| e.eq_ignore_ascii_case(ext))
}

pub fn walk_files(root: &str, exts: &[Arc<str>]) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let root_path = Path::new(root);
    let mut audio = Vec::new();
    let mut playlists = Vec::new();

    if !root_path.is_dir() {
        return (audio, playlists);
    }

    for entry in WalkDir::new(root_path).follow_links(true) {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path().to_path_buf();
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
        let ext_with_dot = if ext.is_empty() { String::new() } else { format!(".{}", ext) };
        if is_supported_audio(&ext_with_dot, exts) {
            audio.push(path);
        } else if PLAYLIST_EXTS.contains(&ext_with_dot) {
            playlists.push(path);
        }
    }
    audio.sort();
    playlists.sort();
    (audio, playlists)
}

fn get_or_insert_artist(conn: &Connection, name: &str) -> Result<i64, String> {
    if name.is_empty() {
        return Ok(0);
    }
    let normalized = normalize_artist_name(name);

    if let Ok(id) = conn.query_row("SELECT id FROM artists WHERE normalized_name = ?", [&normalized], |row| row.get::<usize, i64>(0)) {
        return Ok(id);
    }
    if let Ok(id) = conn.query_row("SELECT id FROM artists WHERE name = ?", [name], |row| row.get::<usize, i64>(0)) {
        conn.execute("UPDATE artists SET normalized_name = ? WHERE id = ?", params![&normalized, id])
            .map_err(|e| e.to_string())?;
        return Ok(id);
    }
    conn.execute(
        "INSERT INTO artists (name, normalized_name, created_at) VALUES (?, ?, ?)",
        params![name, &normalized, Utc::now().to_rfc3339()],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

fn get_or_insert_album(
    conn: &Connection,
    name: &str,
    album_artist: Option<&str>,
    artist_id: Option<i64>,
    year: Option<i64>,
    has_cover: bool,
) -> Result<i64, String> {
    let aa = album_artist.unwrap_or("");
    let existing = conn
        .query_row(
            "SELECT id FROM albums WHERE name = ? AND COALESCE(album_artist, '') = ?",
            params![name, aa],
            |row| row.get::<usize, i64>(0),
        )
        .ok();

    if let Some(id) = existing {
        if has_cover {
            conn.execute(
                "UPDATE albums SET has_cover = 1, year = COALESCE(?, year), artist_id = COALESCE(?, artist_id) WHERE id = ?",
                params![year, artist_id, id],
            )
            .map_err(|e| e.to_string())?;
        } else {
            conn.execute(
                "UPDATE albums SET year = COALESCE(?, year), artist_id = COALESCE(?, artist_id) WHERE id = ?",
                params![year, artist_id, id],
            )
            .map_err(|e| e.to_string())?;
        }
        return Ok(id);
    }

    conn.execute(
        "INSERT INTO albums (name, album_artist, artist_id, year, has_cover) VALUES (?, ?, ?, ?, ?)",
        params![name, aa, artist_id, year, if has_cover { 1 } else { 0 }],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

fn sync_track_artists(conn: &Connection, track_id: i64, raw_artist: Option<&str>) -> Result<i64, String> {
    let parsed = split_artists(raw_artist);
    conn.execute("DELETE FROM track_artists WHERE track_id = ?", [track_id])
        .map_err(|e| e.to_string())?;

    if parsed.is_empty() {
        let unknown = normalize_artist_name("Unknown Artist");
        let id = match conn.query_row("SELECT id FROM artists WHERE normalized_name = ?", [&unknown], |row| row.get::<usize, i64>(0)) {
            Ok(id) => id,
            Err(_) => {
                conn.execute(
                    "INSERT INTO artists (name, normalized_name, created_at) VALUES (?, ?, ?)",
                    params!["Unknown Artist", &unknown, Utc::now().to_rfc3339()],
                )
                .map_err(|e| e.to_string())?;
                conn.last_insert_rowid()
            }
        };
        conn.execute(
            "INSERT OR REPLACE INTO track_artists (track_id, artist_id, role, position) VALUES (?, ?, ?, ?)",
            params![track_id, id, "primary", 0],
        )
        .map_err(|e| e.to_string())?;
        return Ok(id);
    }

    let mut primary_id: Option<i64> = None;
    for pa in &parsed {
        let artist_id = get_or_insert_artist(conn, &pa.name)?;
        conn.execute(
            "INSERT OR REPLACE INTO track_artists (track_id, artist_id, role, position) VALUES (?, ?, ?, ?)",
            params![track_id, artist_id, &pa.role, pa.position as i64],
        )
        .map_err(|e| e.to_string())?;
        if pa.role == "primary" && primary_id.is_none() {
            primary_id = Some(artist_id);
        }
    }

    Ok(primary_id.unwrap_or_else(|| get_or_insert_artist(conn, &parsed[0].name).unwrap_or(0)))
}

fn get_primary_artist_id(conn: &Connection, raw_artist: Option<&str>) -> Result<i64, String> {
    let parsed = split_artists(raw_artist);
    if parsed.is_empty() {
        let unknown = normalize_artist_name("Unknown Artist");
        return match conn.query_row("SELECT id FROM artists WHERE normalized_name = ?", [&unknown], |row| row.get::<usize, i64>(0)) {
            Ok(id) => Ok(id),
            Err(_) => {
                conn.execute(
                    "INSERT INTO artists (name, normalized_name, created_at) VALUES (?, ?, ?)",
                    params!["Unknown Artist", &unknown, Utc::now().to_rfc3339()],
                )
                .map_err(|e| e.to_string())?;
                Ok(conn.last_insert_rowid())
            }
        };
    }
    let primary = parsed.iter().find(|p| p.role == "primary").unwrap_or(&parsed[0]);
    get_or_insert_artist(conn, &primary.name)
}

pub fn upsert_track_from_meta(
    conn: &mut Connection,
    file_path: &Path,
    mtime: &str,
) -> Result<&'static str, String> {
    let meta = parse_track_metadata(file_path).ok_or("parse failed")?;
    let filename = file_path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();

    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let primary_artist_id = get_primary_artist_id(&tx, meta.artist.as_deref())?;
    let album_id = match meta.album.as_deref() {
        Some(album) => {
            Some(get_or_insert_album(
                &tx,
                album,
                meta.album_artist.as_deref(),
                Some(primary_artist_id).filter(|x| *x != 0),
                meta.year,
                meta.has_cover,
            )?)
        }
        None => None,
    };

    let existing = tx
        .query_row(
            "SELECT id, file_modified_at FROM tracks WHERE path = ?",
            [file_path.to_string_lossy().as_ref()],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?)),
        )
        .ok();

    let action;
    if let Some((id, _)) = existing {
        tx.execute(
            "UPDATE tracks SET filename=?, title=?, artist=?, album=?, album_artist=?, album_id=?, artist_id=?,
             duration=?, track_number=?, disk_number=?, format=?, bitrate=?, sample_rate=?,
             has_cover=?, file_modified_at=? WHERE id=?",
            params![
                filename,
                meta.title,
                meta.artist,
                meta.album,
                meta.album_artist,
                album_id,
                Some(primary_artist_id).filter(|x| *x != 0),
                meta.duration,
                meta.track_number,
                meta.disk_number,
                meta.format,
                meta.bitrate,
                meta.sample_rate,
                if meta.has_cover { 1 } else { 0 },
                mtime,
                id,
            ],
        )
        .map_err(|e| e.to_string())?;
        sync_track_artists(&tx, id, meta.artist.as_deref())?;
        action = "updated";
    } else {
        tx.execute(
            "INSERT INTO tracks
             (path, filename, title, artist, album, album_artist, album_id, artist_id,
              duration, track_number, disk_number, format, bitrate, sample_rate, has_cover,
              liked, play_count, date_added, file_modified_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, 0, ?, ?)",
            params![
                file_path.to_string_lossy().as_ref(),
                filename,
                meta.title,
                meta.artist,
                meta.album,
                meta.album_artist,
                album_id,
                Some(primary_artist_id).filter(|x| *x != 0),
                meta.duration,
                meta.track_number,
                meta.disk_number,
                meta.format,
                meta.bitrate,
                meta.sample_rate,
                if meta.has_cover { 1 } else { 0 },
                Utc::now().to_rfc3339(),
                mtime,
            ],
        )
        .map_err(|e| e.to_string())?;
        let id = tx.last_insert_rowid();
        sync_track_artists(&tx, id, meta.artist.as_deref())?;
        action = "added";
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(action)
}

pub fn scan_library(
    pool: &DbPool,
    root_folder: &str,
    supported_extensions: &[Arc<str>],
) -> Result<ScanProgress, String> {
    let (audio_files, playlist_files) = walk_files(root_folder, supported_extensions);
    let mut progress = ScanProgress::default();

    let mut conn = get_conn(pool).map_err(|e| e.to_string())?;

    // Safety net: don't prune if the walk returned nothing but DB has tracks.
    if audio_files.is_empty() {
        let db_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM tracks", [], |row| row.get(0))
            .unwrap_or(0);
        if db_count > 0 {
            log::error!(
                "[scanner] walk of \"{}\" returned 0 audio files but DB has {} tracks — aborting scan to avoid data wipe.",
                root_folder,
                db_count
            );
            return Ok(progress);
        }
    }

    for file in &audio_files {
        progress.scanned += 1;
        let mtime = match std::fs::metadata(file) {
            Ok(m) => m
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| chrono::DateTime::from_timestamp(d.as_secs() as i64, 0).map(|dt| dt.to_rfc3339()).unwrap_or_default())
                .unwrap_or_default(),
            Err(_) => {
                progress.failed += 1;
                continue;
            }
        };
        match upsert_track_from_meta(&mut conn, file, &mtime) {
            Ok("added") => progress.added += 1,
            Ok("updated") => progress.updated += 1,
            _ => progress.failed += 1,
        }
    }

    // prune deleted files
    let known: HashSet<String> = audio_files
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    let mut stmt = conn.prepare("SELECT id, path FROM tracks").map_err(|e| e.to_string())?;
    let all_rows: Vec<(i64, String)> = stmt
        .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)))
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    drop(stmt);
    for (id, path) in &all_rows {
        if !known.contains(path) {
            let _ = conn.execute("DELETE FROM tracks WHERE id = ?", [id]);
        }
    }

    // auto-import playlists
    let known_playlist_sources: HashSet<String> = playlist_files
        .iter()
        .map(|p| std::fs::canonicalize(p).unwrap_or_else(|_| p.clone()).to_string_lossy().into_owned())
        .collect();
    crate::import_playlist::remove_stale_imported_playlists(&conn, &known_playlist_sources)
        .map_err(|e| e.to_string())?;
    for playlist_file in &playlist_files {
        match crate::import_playlist::import_playlist_from_file(&mut conn, playlist_file, false) {
            Ok(r) => {
                if r.total_entries > 0 || r.matched > 0 || r.missing > 0 {
                    progress.playlists_imported += 1;
                }
            }
            Err(e) => log::error!("[scanner] failed to import playlist {}: {}", playlist_file.display(), e),
        }
    }

    Ok(progress)
}

pub fn scan_single_file(pool: &DbPool, file_path: &Path) -> Result<&'static str, String> {
    let ext = file_path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    if PLAYLIST_EXTS.contains(ext.as_str()) {
        return scan_single_playlist(pool, file_path);
    }

    let mut conn = get_conn(pool).map_err(|e| e.to_string())?;
    let mtime = match std::fs::metadata(file_path) {
        Ok(m) => m
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| chrono::DateTime::from_timestamp(d.as_secs() as i64, 0).map(|dt| dt.to_rfc3339()).unwrap_or_default())
            .unwrap_or_default(),
        Err(_) => return Ok("failed"),
    };
    match upsert_track_from_meta(&mut conn, file_path, &mtime) {
        Ok(a) => Ok(a),
        Err(_) => Ok("failed"),
    }
}

fn scan_single_playlist(pool: &DbPool, file_path: &Path) -> Result<&'static str, String> {
    let mut conn = get_conn(pool).map_err(|e| e.to_string())?;
    let abs = std::fs::canonicalize(file_path).unwrap_or_else(|_| file_path.to_path_buf());
    let existing = conn
        .query_row(
            "SELECT id FROM playlists WHERE source = ? AND is_imported = 1",
            [abs.to_string_lossy().as_ref()],
            |row| row.get::<_, i64>(0),
        )
        .ok();
    crate::import_playlist::import_playlist_from_file(&mut conn, file_path, existing.is_some())
        .map_err(|e| e.to_string())?;
    Ok(if existing.is_some() { "updated" } else { "added" })
}

pub fn remove_track_by_path(pool: &DbPool, file_path: &str) {
    if let Ok(conn) = get_conn(pool) {
        let _ = conn.execute("DELETE FROM tracks WHERE path = ?", [file_path]);
    }
}

pub fn remove_playlist_by_path(pool: &DbPool, file_path: &str) {
    if let Ok(conn) = get_conn(pool) {
        let abs = std::fs::canonicalize(file_path).unwrap_or_else(|_| PathBuf::from(file_path));
        let _ = conn.execute(
            "DELETE FROM playlists WHERE source = ? AND is_imported = 1",
            [abs.to_string_lossy().as_ref()],
        );
    }
}
