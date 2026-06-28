//! Axum route handlers. These mirror the original Express routers in
//! `server/src/routes/*.ts`, exposing the same `/api/*` surface.

use std::path::PathBuf;

use axum::extract::{Path as AxumPath, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{delete, get, post};
use axum::Router;
use rusqlite::{params, Connection};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::config::{load_config, patch_config};
use crate::db::{attach_track_artists, get_conn, row_to_track};
use crate::metadata::{read_track_metadata, update_track_metadata, MetadataUpdate};
use crate::scanner::scan_library;
use crate::types::*;
use crate::AppState;

// ── helpers ─────────────────────────────────────────────────

fn err(status: StatusCode, msg: impl Into<String>) -> Response {
    (status, Json(json!({ "error": msg.into() }))).into_response()
}

fn clamp_int(v: Option<&str>, min: i64, max: i64, dflt: i64) -> i64 {
    match v.and_then(|s| s.parse::<i64>().ok()) {
        Some(n) => n.clamp(min, max),
        None => dflt,
    }
}

fn like_pattern(q: &str) -> String {
    format!("%{}%", q.replace('%', "\\%"))
}

fn count_rows(conn: &rusqlite::Connection, table: &str) -> i64 {
    conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row.get(0))
        .unwrap_or(0)
}

// ════════════════════════════════════════════════════════════
// LIBRARY ROUTES
// ════════════════════════════════════════════════════════════

#[derive(Deserialize)]
struct TracksQuery {
    q: Option<String>,
    limit: Option<String>,
    offset: Option<String>,
}

async fn get_tracks(State(state): State<AppState>, Query(q): Query<TracksQuery>) -> Response {
    let pool = &state.pool;
    let query = q.q.as_deref().unwrap_or("").trim().to_string();
    let limit = clamp_int(q.limit.as_deref(), 1, 2000, 500);
    let offset = clamp_int(q.offset.as_deref(), 0, 1_000_000, 0);

    let Ok(conn) = get_conn(pool) else {
        return err(StatusCode::INTERNAL_SERVER_ERROR, "db error");
    };

    let mut tracks: Vec<Track> = if !query.is_empty() {
        let like = like_pattern(&query);
        let mut stmt = match conn.prepare(
            "SELECT * FROM tracks WHERE title LIKE ? OR artist LIKE ? OR album LIKE ? ORDER BY title COLLATE NOCASE LIMIT ? OFFSET ?",
        ) {
            Ok(s) => s,
            Err(e) => return err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        };
        let rows = stmt
            .query_map(params![&like, &like, &like, limit, offset], |row| row_to_track(row))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect::<Vec<_>>();
        rows
    } else {
        let mut stmt = match conn.prepare("SELECT * FROM tracks ORDER BY date_added DESC LIMIT ? OFFSET ?") {
            Ok(s) => s,
            Err(e) => return err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        };
        stmt.query_map(params![limit, offset], |row| row_to_track(row))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect::<Vec<_>>()
    };

    if let Err(e) = attach_track_artists(&conn, &mut tracks) {
        log::warn!("attach_track_artists: {e}");
    }
    let total = count_rows(&conn, "tracks");
    Json(json!({ "items": tracks, "total": total })).into_response()
}

async fn search(State(state): State<AppState>, Query(q): Query<TracksQuery>) -> Response {
    let pool = &state.pool;
    let query = match &q.q {
        Some(s) if !s.trim().is_empty() => s.trim().to_string(),
        _ => return Json(json!({ "tracks": [], "albums": [], "artists": [] })).into_response(),
    };
    let like = like_pattern(&query);

    let Ok(conn) = get_conn(pool) else {
        return err(StatusCode::INTERNAL_SERVER_ERROR, "db error");
    };

    // tracks
    let mut tracks: Vec<Track> = {
        let mut stmt = conn.prepare(
            "SELECT DISTINCT t.* FROM tracks t
             LEFT JOIN track_artists ta ON ta.track_id = t.id
             LEFT JOIN artists a ON a.id = ta.artist_id
             WHERE t.title LIKE ? OR t.artist LIKE ? OR t.album LIKE ? OR a.name LIKE ?
             ORDER BY t.title COLLATE NOCASE LIMIT 50",
        ).unwrap();
        stmt.query_map(params![&like, &like, &like, &like], |row| row_to_track(row))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect::<Vec<_>>()
    };
    let _ = attach_track_artists(&conn, &mut tracks);

    // albums
    let albums: Vec<Value> = {
        let mut stmt = conn.prepare(
            "SELECT a.id, a.name, a.album_artist AS albumArtist, a.artist_id AS artistId,
                    a.year, a.has_cover AS hasCover,
                    (SELECT COUNT(*) FROM tracks t WHERE t.album_id = a.id) AS trackCount,
                    (SELECT COALESCE((SELECT t.id FROM tracks t WHERE t.album_id = a.id AND t.has_cover = 1 LIMIT 1), (SELECT t.id FROM tracks t WHERE t.album_id = a.id LIMIT 1))) AS coverTrackId
             FROM albums a
             WHERE a.name LIKE ? OR a.album_artist LIKE ?
             ORDER BY a.name COLLATE NOCASE LIMIT 20",
        ).unwrap();
        stmt.query_map(params![&like, &like], |row| {
            Ok(json!({
                "id": row.get::<usize, i64>(0)?,
                "name": row.get::<usize, String>(1)?,
                "albumArtist": row.get::<usize, Option<String>>(2)?,
                "artistId": row.get::<usize, Option<i64>>(3)?,
                "year": row.get::<usize, Option<i64>>(4)?,
                "hasCover": row.get::<usize, Option<i64>>(5)? == Some(1),
                "trackCount": row.get::<usize, i64>(6)?,
                "coverTrackId": row.get::<usize, Option<i64>>(7)?,
            }))
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect::<Vec<_>>()
    };

    // artists
    let artists: Vec<Value> = {
        let mut stmt = conn.prepare(
            "SELECT a.id, a.name, a.image_path AS imagePath,
                    (SELECT COUNT(DISTINCT t.album_id) FROM track_artists ta
                     JOIN tracks t ON t.id = ta.track_id WHERE ta.artist_id = a.id) AS albumCount,
                    (SELECT COUNT(*) FROM track_artists ta WHERE ta.artist_id = a.id) AS trackCount
             FROM artists a
             WHERE a.name LIKE ?
             ORDER BY
               CASE WHEN a.normalized_name = 'unknown artist' THEN 1 ELSE 0 END,
               a.name COLLATE NOCASE
             LIMIT 20",
        ).unwrap();
        stmt.query_map([&like], |row| {
            Ok(json!({
                "id": row.get::<usize, i64>(0)?,
                "name": row.get::<usize, String>(1)?,
                "imagePath": row.get::<usize, Option<String>>(2)?,
                "albumCount": row.get::<usize, i64>(3)?,
                "trackCount": row.get::<usize, i64>(4)?,
            }))
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect::<Vec<_>>()
    };

    Json(json!({ "tracks": tracks, "albums": albums, "artists": artists })).into_response()
}

async fn get_albums(State(state): State<AppState>) -> Response {
    let Ok(conn) = get_conn(&state.pool) else {
        return err(StatusCode::INTERNAL_SERVER_ERROR, "db error");
    };
    let mut stmt = match conn.prepare(
        "SELECT a.id, a.name, a.album_artist AS albumArtist, a.artist_id AS artistId,
                a.year, a.has_cover AS hasCover,
                (SELECT COUNT(*) FROM tracks t WHERE t.album_id = a.id) AS trackCount,
                (SELECT COALESCE((SELECT t.id FROM tracks t WHERE t.album_id = a.id AND t.has_cover = 1 LIMIT 1), (SELECT t.id FROM tracks t WHERE t.album_id = a.id LIMIT 1))) AS coverTrackId
         FROM albums a ORDER BY a.name COLLATE NOCASE",
    ) {
        Ok(s) => s,
        Err(e) => return err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    let items: Vec<Value> = stmt
        .query_map([], |row| {
            Ok(json!({
                "id": row.get::<usize, i64>(0)?,
                "name": row.get::<usize, String>(1)?,
                "albumArtist": row.get::<usize, Option<String>>(2)?,
                "artistId": row.get::<usize, Option<i64>>(3)?,
                "year": row.get::<usize, Option<i64>>(4)?,
                "hasCover": row.get::<usize, Option<i64>>(5)? == Some(1),
                "trackCount": row.get::<usize, i64>(6)?,
                "coverTrackId": row.get::<usize, Option<i64>>(7)?,
            }))
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    Json(json!({ "items": items })).into_response()
}

async fn get_album(State(state): State<AppState>, AxumPath(id): AxumPath<i64>) -> Response {
    let Ok(conn) = get_conn(&state.pool) else {
        return err(StatusCode::INTERNAL_SERVER_ERROR, "db error");
    };
    let album = conn.query_row(
        "SELECT a.id, a.name, a.album_artist AS albumArtist, a.artist_id AS artistId,
                a.year, a.has_cover AS hasCover,
                (SELECT COUNT(*) FROM tracks t WHERE t.album_id = a.id) AS trackCount,
                (SELECT COALESCE((SELECT t.id FROM tracks t WHERE t.album_id = a.id AND t.has_cover = 1 LIMIT 1), (SELECT t.id FROM tracks t WHERE t.album_id = a.id LIMIT 1))) AS coverTrackId
         FROM albums a WHERE a.id = ?",
        [id],
        |row| {
            Ok(json!({
                "id": row.get::<usize, i64>(0)?,
                "name": row.get::<usize, String>(1)?,
                "albumArtist": row.get::<usize, Option<String>>(2)?,
                "artistId": row.get::<usize, Option<i64>>(3)?,
                "year": row.get::<usize, Option<i64>>(4)?,
                "hasCover": row.get::<usize, Option<i64>>(5)? == Some(1),
                "trackCount": row.get::<usize, i64>(6)?,
                "coverTrackId": row.get::<usize, Option<i64>>(7)?,
            }))
        },
    );
    let album = match album {
        Ok(a) => a,
        Err(_) => return err(StatusCode::NOT_FOUND, "Album not found"),
    };
    let mut stmt = conn.prepare(
        "SELECT * FROM tracks WHERE album_id = ? ORDER BY COALESCE(disk_number, 1), COALESCE(track_number, 0), title COLLATE NOCASE",
    ).unwrap();
    let mut tracks: Vec<Track> = stmt
        .query_map([id], |row| row_to_track(row))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    let _ = attach_track_artists(&conn, &mut tracks);
    Json(json!({ "album": album, "tracks": tracks })).into_response()
}

// ── artists ─────────────────────────────────────────────────

async fn get_artists(State(state): State<AppState>) -> Response {
    let Ok(conn) = get_conn(&state.pool) else {
        return err(StatusCode::INTERNAL_SERVER_ERROR, "db error");
    };
    let mut stmt = match conn.prepare(
        "SELECT a.id, a.name, a.image_path AS imagePath,
                (SELECT COUNT(DISTINCT t.album_id) FROM track_artists ta
                 JOIN tracks t ON t.id = ta.track_id
                 WHERE ta.artist_id = a.id AND ta.role = 'primary') AS albumCount,
                (SELECT COUNT(*) FROM track_artists ta
                 WHERE ta.artist_id = a.id AND ta.role = 'primary') AS primaryTrackCount,
                (SELECT COUNT(*) FROM track_artists ta WHERE ta.artist_id = a.id) AS trackCount
         FROM artists a
         ORDER BY
           CASE WHEN a.normalized_name = 'unknown artist' THEN 1 ELSE 0 END,
           a.name COLLATE NOCASE",
    ) {
        Ok(s) => s,
        Err(e) => return err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    let items: Vec<Value> = stmt
        .query_map([], |row| {
            Ok(json!({
                "id": row.get::<usize, i64>(0)?,
                "name": row.get::<usize, String>(1)?,
                "imagePath": row.get::<usize, Option<String>>(2)?,
                "albumCount": row.get::<usize, i64>(3)?,
                "primaryTrackCount": row.get::<usize, i64>(4)?,
                "trackCount": row.get::<usize, i64>(5)?,
            }))
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    Json(json!({ "items": items })).into_response()
}

async fn get_artist(State(state): State<AppState>, AxumPath(id): AxumPath<i64>) -> Response {
    let Ok(conn) = get_conn(&state.pool) else {
        return err(StatusCode::INTERNAL_SERVER_ERROR, "db error");
    };
    let artist = match conn.query_row(
        "SELECT id, name, image_path AS imagePath, created_at AS createdAt FROM artists WHERE id = ?",
        [id],
        |row| {
            Ok(json!({
                "id": row.get::<usize, i64>(0)?,
                "name": row.get::<usize, String>(1)?,
                "imagePath": row.get::<usize, Option<String>>(2)?,
                "createdAt": row.get::<usize, String>(3)?,
            }))
        },
    ) {
        Ok(a) => a,
        Err(_) => return err(StatusCode::NOT_FOUND, "Artist not found"),
    };

    let albums = query_albums(&conn,
        "SELECT DISTINCT al.id, al.name, al.album_artist AS albumArtist, al.year,
                al.has_cover AS hasCover,
                (SELECT COUNT(*) FROM tracks t WHERE t.album_id = al.id) AS trackCount,
                (SELECT COALESCE((SELECT t.id FROM tracks t WHERE t.album_id = al.id AND t.has_cover = 1 LIMIT 1), (SELECT t.id FROM tracks t WHERE t.album_id = al.id LIMIT 1))) AS coverTrackId
         FROM albums al
         WHERE al.id IN (
           SELECT DISTINCT t.album_id FROM track_artists ta
           JOIN tracks t ON t.id = ta.track_id
           WHERE ta.artist_id = ? AND ta.role = 'primary' AND t.album_id IS NOT NULL
         )
         ORDER BY al.year DESC NULLS LAST, al.name COLLATE NOCASE", params![id]);

    let appears_on = query_albums(&conn,
        "SELECT DISTINCT al.id, al.name, al.album_artist AS albumArtist, al.year,
                al.has_cover AS hasCover,
                (SELECT COUNT(*) FROM tracks t WHERE t.album_id = al.id) AS trackCount,
                (SELECT COALESCE((SELECT t.id FROM tracks t WHERE t.album_id = al.id AND t.has_cover = 1 LIMIT 1), (SELECT t.id FROM tracks t WHERE t.album_id = al.id LIMIT 1))) AS coverTrackId
         FROM albums al
         WHERE al.id IN (
           SELECT DISTINCT t.album_id FROM track_artists ta
           JOIN tracks t ON t.id = ta.track_id
           WHERE ta.artist_id = ? AND ta.role = 'featured' AND t.album_id IS NOT NULL
         )
         ORDER BY al.name COLLATE NOCASE", params![id]);

    let primary_tracks = query_role_tracks(&conn, id, "primary");
    let featured_tracks = query_role_tracks(&conn, id, "featured");

    Json(json!({
        "artist": artist,
        "albums": albums,
        "appearsOn": appears_on,
        "primaryTracks": primary_tracks,
        "featuredTracks": featured_tracks,
        "totalTrackCount": primary_tracks.len() + featured_tracks.len(),
    })).into_response()
}

async fn get_artist_tracks(State(state): State<AppState>, AxumPath(id): AxumPath<i64>) -> Response {
    let Ok(conn) = get_conn(&state.pool) else {
        return err(StatusCode::INTERNAL_SERVER_ERROR, "db error");
    };
    let exists: bool = conn.query_row("SELECT 1 FROM artists WHERE id = ?", [id], |_| Ok(true)).unwrap_or(false);
    if !exists {
        return err(StatusCode::NOT_FOUND, "Artist not found");
    }
    let mut stmt = conn.prepare(
        "SELECT t.*, ta.role FROM track_artists ta
         JOIN tracks t ON t.id = ta.track_id
         WHERE ta.artist_id = ?
         ORDER BY ta.role, t.album COLLATE NOCASE, COALESCE(t.disk_number, 1), COALESCE(t.track_number, 0)",
    ).unwrap();
    let tracks_raw: Vec<(Track, String)> = stmt
        .query_map([id], |row| {
            let t = row_to_track(row)?;
            let role: String = row.get("role")?;
            Ok((t, role))
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    let mut tracks_only: Vec<Track> = tracks_raw.iter().map(|(t, _)| t.clone()).collect();
    let _ = attach_track_artists(&conn, &mut tracks_only);
    let items: Vec<Value> = tracks_only
        .into_iter()
        .zip(tracks_raw.into_iter().map(|(_, role)| role))
        .map(|(t, role)| {
            let mut v = serde_json::to_value(&t).unwrap();
            if let Some(obj) = v.as_object_mut() {
                obj.insert("role".to_string(), Value::String(role));
            }
            v
        })
        .collect();
    Json(json!({ "items": items })).into_response()
}

// ── liked ───────────────────────────────────────────────────

async fn get_liked(State(state): State<AppState>) -> Response {
    let Ok(conn) = get_conn(&state.pool) else {
        return err(StatusCode::INTERNAL_SERVER_ERROR, "db error");
    };
    let mut stmt = conn.prepare("SELECT * FROM tracks WHERE liked = 1 ORDER BY date_added DESC").unwrap();
    let mut tracks: Vec<Track> = stmt.query_map([], |row| row_to_track(row)).unwrap().filter_map(|r| r.ok()).collect();
    let _ = attach_track_artists(&conn, &mut tracks);
    Json(json!({ "items": tracks })).into_response()
}

async fn toggle_like(State(state): State<AppState>, AxumPath(track_id): AxumPath<i64>, body: Json<Value>) -> Response {
    let liked = body.0.get("liked").and_then(|v| v.as_bool()).unwrap_or(false);
    let Ok(conn) = get_conn(&state.pool) else {
        return err(StatusCode::INTERNAL_SERVER_ERROR, "db error");
    };
    let n = conn.execute("UPDATE tracks SET liked = ? WHERE id = ?", params![liked as i32, track_id]).unwrap_or(0);
    if n == 0 {
        return err(StatusCode::NOT_FOUND, "Track not found");
    }
    Json(json!({ "id": track_id, "liked": liked })).into_response()
}

async fn mark_played(State(state): State<AppState>, AxumPath(track_id): AxumPath<i64>) -> Response {
    let Ok(conn) = get_conn(&state.pool) else {
        return err(StatusCode::INTERNAL_SERVER_ERROR, "db error");
    };
    let n = conn.execute("UPDATE tracks SET play_count = play_count + 1 WHERE id = ?", [track_id]).unwrap_or(0);
    if n == 0 {
        return err(StatusCode::NOT_FOUND, "Track not found");
    }
    Json(json!({ "ok": true })).into_response()
}

// ── metadata editor ─────────────────────────────────────────

async fn get_track_metadata(State(state): State<AppState>, AxumPath(track_id): AxumPath<i64>) -> Response {
    let Ok(conn) = get_conn(&state.pool) else {
        return err(StatusCode::INTERNAL_SERVER_ERROR, "db error");
    };
    let path = match conn.query_row("SELECT path FROM tracks WHERE id = ?", [track_id], |row| row.get::<usize, String>(0)) {
        Ok(p) => PathBuf::from(p),
        Err(_) => return err(StatusCode::NOT_FOUND, "Track not found"),
    };
    match read_track_metadata(&path) {
        Ok(meta) => Json(meta).into_response(),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, e),
    }
}

async fn patch_track_metadata(
    State(state): State<AppState>,
    AxumPath(track_id): AxumPath<i64>,
    body: Json<Value>,
) -> Response {
    let Ok(conn) = get_conn(&state.pool) else {
        return err(StatusCode::INTERNAL_SERVER_ERROR, "db error");
    };
    let path = match conn.query_row("SELECT path FROM tracks WHERE id = ?", [track_id], |row| row.get::<usize, String>(0)) {
        Ok(p) => PathBuf::from(p),
        Err(_) => return err(StatusCode::NOT_FOUND, "Track not found"),
    };
    let v = body.0;
    let cover_art = match v.get("coverArt") {
        Some(Value::Null) => Some(None),
        Some(obj) => Some(Some(crate::types::CoverArt {
            mime_type: obj.get("mimeType").and_then(|m| m.as_str()).unwrap_or("image/jpeg").to_string(),
            data: obj.get("data").and_then(|d| d.as_str()).unwrap_or("").to_string(),
        })),
        None => None,
    };
    let update = MetadataUpdate {
        title: v.get("title").and_then(|x| x.as_str()).map(|s| s.to_string()),
        artist: v.get("artist").and_then(|x| x.as_str()).map(|s| s.to_string()),
        album: v.get("album").and_then(|x| x.as_str()).map(|s| s.to_string()),
        album_artist: v.get("albumArtist").and_then(|x| x.as_str()).map(|s| s.to_string()),
        year: v.get("year").and_then(|x| if x.is_null() { Some(None) } else { x.as_i64().map(Some) }),
        track_number: v.get("trackNumber").and_then(|x| if x.is_null() { Some(None) } else { x.as_i64().map(Some) }),
        cover_art,
    };
    if let Err(e) = update_track_metadata(&path, &update) {
        return err(StatusCode::INTERNAL_SERVER_ERROR, e);
    }
    // rescan so the DB reflects the new metadata
    if let Err(e) = crate::scanner::scan_single_file(&state.pool, &path) {
        return err(StatusCode::INTERNAL_SERVER_ERROR, format!("Tags written but DB rescan failed: {e}"));
    }
    Json(json!({ "trackId": track_id, "path": path.to_string_lossy() })).into_response()
}

// ── query helpers (shared by artist detail) ─────────────────

fn query_albums(conn: &rusqlite::Connection, sql: &str, args: impl rusqlite::Params) -> Vec<Value> {
    let mut stmt = match conn.prepare(sql) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    stmt.query_map(args, |row| {
        Ok(json!({
            "id": row.get::<usize, i64>(0)?,
            "name": row.get::<usize, String>(1)?,
            "albumArtist": row.get::<usize, Option<String>>(2)?,
            "year": row.get::<usize, Option<i64>>(3)?,
            "hasCover": row.get::<usize, Option<i64>>(4)? == Some(1),
            "trackCount": row.get::<usize, i64>(5)?,
            "coverTrackId": row.get::<usize, Option<i64>>(6)?,
        }))
    })
    .unwrap()
    .filter_map(|r| r.ok())
    .collect()
}

fn query_role_tracks(conn: &rusqlite::Connection, artist_id: i64, role: &str) -> Vec<Track> {
    let sql = format!(
        "SELECT t.* FROM track_artists ta
         JOIN tracks t ON t.id = ta.track_id
         WHERE ta.artist_id = ? AND ta.role = '{}'
         ORDER BY t.album COLLATE NOCASE, COALESCE(t.disk_number, 1), COALESCE(t.track_number, 0)",
        role
    );
    let mut stmt = match conn.prepare(&sql) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    let mut tracks: Vec<Track> = stmt
        .query_map([artist_id], |row| row_to_track(row))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    let _ = attach_track_artists(conn, &mut tracks);
    tracks
}

// ════════════════════════════════════════════════════════════
// PLAYLIST ROUTES
// ════════════════════════════════════════════════════════════

fn shape_playlist_value(id: i64, name: &str, description: Option<&str>, created_at: &str, is_imported: bool, source: Option<&str>, position: Option<i64>, track_count: i64) -> Value {
    json!({
        "id": id,
        "name": name,
        "description": description,
        "trackCount": track_count,
        "createdAt": created_at,
        "isImported": is_imported,
        "source": source,
        "position": position,
    })
}

/// Write an imported playlist's current DB state back to its source .m3u file.
/// Only fires for playlists with `is_imported = 1` and a non-empty `source`.
/// Failures are logged, not fatal — the DB mutation already succeeded.
fn sync_imported_playlist_to_disk(conn: &Connection, playlist_id: i64) {
    let row = conn.query_row(
        "SELECT name, source FROM playlists WHERE id = ? AND is_imported = 1",
        [playlist_id],
        |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?)),
    );
    let (name, source): (String, Option<String>) = match row {
        Ok(x) => x,
        Err(_) => return, // not imported or not found — nothing to sync
    };
    let source_path = match source {
        Some(ref s) if !s.is_empty() => PathBuf::from(s),
        _ => return,
    };
    let base_dir = source_path.parent().map(|p| p.to_path_buf());

    let mut stmt = match conn.prepare(
        "SELECT pe.raw_entry, pe.title, pe.missing, t.path, t.duration, t.artist, t.title AS track_title
         FROM playlist_entries pe
         LEFT JOIN tracks t ON t.id = pe.track_id
         WHERE pe.playlist_id = ?
         ORDER BY pe.position",
    ) {
        Ok(s) => s,
        Err(_) => return,
    };
    let rows: Vec<(String, Option<String>, i64, Option<String>, Option<f64>, Option<String>, Option<String>)> = stmt
        .query_map([playlist_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?))
        })
        .ok()
        .map(|m| m.filter_map(|r| r.ok()).collect())
        .unwrap_or_default();
    drop(stmt);

    use crate::m3u_parser::M3uWriteEntry;
    let mut entries: Vec<M3uWriteEntry> = Vec::with_capacity(rows.len());
    for (raw_entry, ext_title, missing, track_path, duration, artist, track_title) in rows {
        if missing == 1 {
            entries.push(M3uWriteEntry { duration: None, title: if ext_title.is_some() { ext_title } else { None }, path: raw_entry });
            continue;
        }
        let path_line = match track_path {
            Some(ref tp) => {
                let p = PathBuf::from(tp);
                if let Some(ref base) = base_dir {
                    p.strip_prefix(base).map(|r| r.to_string_lossy().into_owned()).unwrap_or_else(|_| tp.clone())
                } else {
                    tp.clone()
                }
            }
            None => raw_entry,
        };
        let title = ext_title
            .or_else(|| match (artist, track_title) {
                (Some(a), Some(t)) => Some(format!("{} - {}", a, t)),
                (Some(a), None) => Some(a),
                (None, Some(t)) => Some(t),
                _ => None,
            });
        entries.push(M3uWriteEntry { duration, title, path: path_line });
    }

    if let Err(e) = crate::m3u_parser::write_m3u_file(&source_path, &name, &entries) {
        log::warn!("[playlist] failed to sync imported playlist {} to {}: {}", playlist_id, source_path.display(), e);
    }
}

async fn get_playlists(State(state): State<AppState>) -> Response {
    let Ok(conn) = get_conn(&state.pool) else {
        return err(StatusCode::INTERNAL_SERVER_ERROR, "db error");
    };
    let mut stmt = match conn.prepare(
        "SELECT id, name, description, created_at, is_imported, source, position FROM playlists ORDER BY COALESCE(position, 999999) ASC, created_at DESC",
    ) {
        Ok(s) => s,
        Err(e) => return err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    let rows: Vec<(i64, String, Option<String>, String, i64, Option<String>, Option<i64>)> = stmt
        .query_map([], |row| {
            Ok((
                row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?,
            ))
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    drop(stmt);
    let items: Vec<Value> = rows
        .iter()
        .map(|(id, name, desc, created, imp, source, pos)| {
            let count: i64 = conn.query_row("SELECT COUNT(*) FROM playlist_entries WHERE playlist_id = ?", [id], |r| r.get(0)).unwrap_or(0);
            shape_playlist_value(*id, name, desc.as_deref(), created, *imp == 1, source.as_deref(), *pos, count)
        })
        .collect();
    Json(json!({ "items": items })).into_response()
}

async fn create_playlist(State(state): State<AppState>, body: Json<Value>) -> Response {
    let name = body.0.get("name").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    if name.is_empty() {
        return err(StatusCode::BAD_REQUEST, "name required");
    }
    let description = body.0.get("description").and_then(|v| v.as_str()).map(|s| s.to_string());
    let Ok(conn) = get_conn(&state.pool) else {
        return err(StatusCode::INTERNAL_SERVER_ERROR, "db error");
    };
    let max_pos: i64 = conn.query_row("SELECT COALESCE(MAX(position), -1) FROM playlists", [], |r| r.get(0)).unwrap_or(-1);
    let created = chrono::Utc::now().to_rfc3339();
    match conn.execute(
        "INSERT INTO playlists (name, description, created_at, is_imported, source, position) VALUES (?, ?, ?, 0, NULL, ?)",
        params![&name, &description, &created, max_pos + 1],
    ) {
        Ok(_) => {
            let id = conn.last_insert_rowid();
            (
                StatusCode::CREATED,
                Json(shape_playlist_value(id, &name, description.as_deref(), &created, false, None, Some(max_pos + 1), 0)),
            )
                .into_response()
        }
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

async fn get_playlist(State(state): State<AppState>, AxumPath(id): AxumPath<i64>) -> Response {
    let Ok(conn) = get_conn(&state.pool) else {
        return err(StatusCode::INTERNAL_SERVER_ERROR, "db error");
    };
    let p = conn.query_row(
        "SELECT id, name, description, created_at, is_imported, source, position FROM playlists WHERE id = ?",
        [id],
        |row| Ok((
            row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, Option<String>>(2)?,
            row.get::<_, String>(3)?, row.get::<_, i64>(4)?, row.get::<_, Option<String>>(5)?, row.get::<_, Option<i64>>(6)?,
        )),
    );
    let (pid, name, desc, created, imp, source, pos) = match p {
        Ok(row) => row,
        Err(_) => return err(StatusCode::NOT_FOUND, "Playlist not found"),
    };
    // entries
    let mut stmt = match conn.prepare(
        "SELECT playlist_id, position, track_id, raw_entry, missing, title FROM playlist_entries WHERE playlist_id = ? ORDER BY position",
    ) {
        Ok(s) => s,
        Err(e) => return err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    let entries: Vec<(i64, i64, Option<i64>, String, i64, Option<String>)> = stmt
        .query_map([id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    drop(stmt);
    // fetch tracks
    let track_ids: Vec<i64> = entries.iter().filter_map(|(_, _, tid, _, _, _)| *tid).collect();
    let mut tracks_map: std::collections::HashMap<i64, Track> = std::collections::HashMap::new();
    if !track_ids.is_empty() {
        let placeholders = track_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!("SELECT * FROM tracks WHERE id IN ({})", placeholders);
        let mut stmt = conn.prepare(&sql).unwrap();
        let mut t: Vec<Track> = stmt.query_map(rusqlite::params_from_iter(&track_ids), |row| row_to_track(row)).unwrap().filter_map(|r| r.ok()).collect();
        drop(stmt);
        let _ = attach_track_artists(&conn, &mut t);
        for tr in t { tracks_map.insert(tr.id, tr); }
    }
    let entries_json: Vec<Value> = entries.iter().map(|(pl_id, pos, tid, raw, missing, title)| {
        json!({
            "playlistId": pl_id,
            "position": pos,
            "trackId": tid,
            "rawEntry": raw,
            "missing": *missing == 1,
            "title": title,
            "track": tid.and_then(|id| tracks_map.get(&id)).map(|t| serde_json::to_value(t).unwrap()),
        })
    }).collect();
    let mut result = shape_playlist_value(pid, &name, desc.as_deref(), &created, imp == 1, source.as_deref(), pos, entries_json.len() as i64);
    if let Some(obj) = result.as_object_mut() {
        obj.insert("entries".to_string(), Value::Array(entries_json));
    }
    Json(result).into_response()
}

async fn rename_playlist(State(state): State<AppState>, AxumPath(id): AxumPath<i64>, body: Json<Value>) -> Response {
    let Ok(conn) = get_conn(&state.pool) else {
        return err(StatusCode::INTERNAL_SERVER_ERROR, "db error");
    };
    let existing = conn.query_row(
        "SELECT name, description FROM playlists WHERE id = ?", [id],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
    );
    let (old_name, old_desc) = match existing {
        Ok(x) => x,
        Err(_) => return err(StatusCode::NOT_FOUND, "Playlist not found"),
    };
    let name = body.0.get("name").and_then(|v| v.as_str()).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).unwrap_or(old_name);
    let desc = if body.0.get("description").is_some() {
        body.0.get("description").and_then(|v| v.as_str()).map(|s| s.to_string())
    } else {
        old_desc
    };
    let _ = conn.execute("UPDATE playlists SET name = ?, description = ? WHERE id = ?", params![&name, &desc, id]);
    sync_imported_playlist_to_disk(&conn, id);
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM playlist_entries WHERE playlist_id = ?", [id], |r| r.get(0)).unwrap_or(0);
    let row = conn.query_row("SELECT created_at, is_imported, source, position FROM playlists WHERE id = ?", [id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?, row.get::<_, Option<String>>(2)?, row.get::<_, Option<i64>>(3)?))
    });
    if let Ok((created, imp, source, pos)) = row {
        Json(shape_playlist_value(id, &name, desc.as_deref(), &created, imp == 1, source.as_deref(), pos, count)).into_response()
    } else {
        err(StatusCode::INTERNAL_SERVER_ERROR, "playlist gone")
    }
}

async fn delete_playlist(State(state): State<AppState>, AxumPath(id): AxumPath<i64>) -> Response {
    let Ok(conn) = get_conn(&state.pool) else {
        return err(StatusCode::INTERNAL_SERVER_ERROR, "db error");
    };
    let _ = conn.execute("DELETE FROM playlists WHERE id = ?", [id]);
    Json(json!({ "ok": true })).into_response()
}

async fn add_tracks_to_playlist(State(state): State<AppState>, AxumPath(id): AxumPath<i64>, body: Json<Value>) -> Response {
    let track_ids: Vec<i64> = body.0.get("trackIds").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|x| x.as_i64()).collect()).unwrap_or_default();
    if track_ids.is_empty() {
        return err(StatusCode::BAD_REQUEST, "trackIds required");
    }
    let Ok(mut conn) = get_conn(&state.pool) else {
        return err(StatusCode::INTERNAL_SERVER_ERROR, "db error");
    };
    let exists = conn.query_row("SELECT 1 FROM playlists WHERE id = ?", [id], |_| Ok(true)).unwrap_or(false);
    if !exists {
        return err(StatusCode::NOT_FOUND, "Playlist not found");
    }
    let max_pos: i64 = conn.query_row("SELECT COALESCE(MAX(position), -1) FROM playlist_entries WHERE playlist_id = ?", [id], |r| r.get(0)).unwrap_or(-1);
    let mut pos = max_pos + 1;
    let mut added = 0i64;
    let tx = conn.transaction().unwrap();
    for tid in &track_ids {
        let filename: Option<String> = tx.query_row("SELECT filename FROM tracks WHERE id = ?", [tid], |r| r.get(0)).ok();
        if filename.is_none() { continue; }
        let already: bool = tx.query_row("SELECT 1 FROM playlist_entries WHERE playlist_id = ? AND track_id = ? LIMIT 1", params![id, tid], |_| Ok(true)).unwrap_or(false);
        if already { continue; }
        let _ = tx.execute("INSERT INTO playlist_entries (playlist_id, position, track_id, raw_entry, missing, title) VALUES (?, ?, ?, ?, 0, NULL)", params![id, pos, tid, &filename.unwrap()]);
        pos += 1;
        added += 1;
    }
    let _ = tx.commit();
    sync_imported_playlist_to_disk(&conn, id);
    (StatusCode::CREATED, Json(json!({ "added": added }))).into_response()
}

async fn remove_track_from_playlist(State(state): State<AppState>, AxumPath(params): AxumPath<(i64, i64)>) -> Response {
    let (id, track_id) = params;
    let Ok(mut conn) = get_conn(&state.pool) else {
        return err(StatusCode::INTERNAL_SERVER_ERROR, "db error");
    };
    let positions: Vec<i64> = conn.prepare("SELECT position FROM playlist_entries WHERE playlist_id = ? AND track_id = ? ORDER BY position")
        .unwrap()
        .query_map(params![id, track_id], |r| r.get(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    if positions.is_empty() {
        return err(StatusCode::NOT_FOUND, "Entry not found");
    }
    let tx = conn.transaction().unwrap();
    let _ = tx.execute("DELETE FROM playlist_entries WHERE playlist_id = ? AND track_id = ?", params![id, track_id]);
    let remaining: Vec<i64> = tx.prepare("SELECT position FROM playlist_entries WHERE playlist_id = ? ORDER BY position")
        .unwrap()
        .query_map([id], |r| r.get(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    for (i, old_pos) in remaining.iter().enumerate() {
        let _ = tx.execute("UPDATE playlist_entries SET position = ? WHERE playlist_id = ? AND position = ?", params![i as i64, id, old_pos]);
    }
    let _ = tx.commit();
    sync_imported_playlist_to_disk(&conn, id);
    Json(json!({ "ok": true })).into_response()
}

async fn reorder_playlist(State(state): State<AppState>, AxumPath(id): AxumPath<i64>, body: Json<Value>) -> Response {
    let from = body.0.get("fromPosition").and_then(|v| v.as_i64());
    let to = body.0.get("toPosition").and_then(|v| v.as_i64());
    let (from, to) = match (from, to) {
        (Some(f), Some(t)) if f != t => (f, t),
        _ => return err(StatusCode::BAD_REQUEST, "fromPosition and toPosition required"),
    };
    let Ok(mut conn) = get_conn(&state.pool) else {
        return err(StatusCode::INTERNAL_SERVER_ERROR, "db error");
    };
    let mut positions: Vec<i64> = conn.prepare("SELECT position FROM playlist_entries WHERE playlist_id = ? ORDER BY position")
        .unwrap()
        .query_map([id], |r| r.get(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    if from < 0 || from as usize >= positions.len() || to < 0 || to as usize >= positions.len() {
        return err(StatusCode::BAD_REQUEST, "Invalid position");
    }
    let moved = positions.remove(from as usize);
    positions.insert(to as usize, moved);
    const OFFSET: i64 = 1_000_000_000;
    let tx = conn.transaction().unwrap();
    for old in &positions { let _ = tx.execute("UPDATE playlist_entries SET position = position + ? WHERE playlist_id = ? AND position = ?", params![OFFSET, id, old]); }
    for (idx, old) in positions.iter().enumerate() { let _ = tx.execute("UPDATE playlist_entries SET position = ? WHERE playlist_id = ? AND position = ?", params![idx as i64, id, old + OFFSET]); }
    if tx.commit().is_ok() {
        sync_imported_playlist_to_disk(&conn, id);
    }
    Json(json!({ "ok": true })).into_response()
}

async fn reorder_playlists(State(state): State<AppState>, body: Json<Value>) -> Response {
    let from = body.0.get("fromPosition").and_then(|v| v.as_i64());
    let to = body.0.get("toPosition").and_then(|v| v.as_i64());
    let (from, to) = match (from, to) {
        (Some(f), Some(t)) if f >= 0 && f != t => (f, t),
        _ => return err(StatusCode::BAD_REQUEST, "fromPosition and toPosition required"),
    };
    let Ok(mut conn) = get_conn(&state.pool) else {
        return err(StatusCode::INTERNAL_SERVER_ERROR, "db error");
    };
    let mut all: Vec<(i64, Option<i64>)> = conn.prepare("SELECT id, position FROM playlists ORDER BY COALESCE(position, 999999) ASC, created_at DESC")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    if from as usize >= all.len() {
        return err(StatusCode::BAD_REQUEST, "Invalid fromPosition");
    }
    let moved = all.remove(from as usize);
    let target = (to as usize).min(all.len());
    all.insert(target, moved);
    let tx = conn.transaction().unwrap();
    for (idx, (pid, _)) in all.iter().enumerate() {
        let _ = tx.execute("UPDATE playlists SET position = ? WHERE id = ?", params![idx as i64, pid]);
    }
    let _ = tx.commit();
    Json(json!({ "ok": true })).into_response()
}



// ════════════════════════════════════════════════════════════
// PLAYBACK ROUTES (stream + cover)
// ════════════════════════════════════════════════════════════

fn mime_for_ext(ext: &str) -> &'static str {
    match ext {
        "mp3" => "audio/mpeg",
        "flac" => "audio/flac",
        "wav" => "audio/wav",
        "m4a" | "mp4" => "audio/mp4",
        "ogg" | "opus" => "audio/ogg",
        _ => "application/octet-stream",
    }
}

async fn stream_track(State(state): State<AppState>, AxumPath(track_id): AxumPath<i64>, headers: HeaderMap) -> Response {
    let Ok(conn) = get_conn(&state.pool) else {
        return err(StatusCode::INTERNAL_SERVER_ERROR, "db error");
    };
    let path = match conn.query_row("SELECT path FROM tracks WHERE id = ?", [track_id], |row| row.get::<usize, String>(0)) {
        Ok(p) => std::path::PathBuf::from(p),
        Err(_) => return err(StatusCode::NOT_FOUND, "Track not found"),
    };
    if !path.exists() {
        return err(StatusCode::GONE, "File gone from disk");
    }
    let file_size = match std::fs::metadata(&path) {
        Ok(m) => m.len(),
        Err(_) => return err(StatusCode::INTERNAL_SERVER_ERROR, "cannot stat file"),
    };
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
    let mime = mime_for_ext(&ext);

    let range = headers.get("range").and_then(|v| v.to_str().ok());
    if let Some(range) = range {
        let parts: Vec<&str> = range.trim().strip_prefix("bytes=").unwrap_or("").split('-').collect();
        let start_raw = parts.first().and_then(|s| if s.is_empty() { None } else { s.parse::<u64>().ok() });
        let end_raw = parts.get(1).and_then(|s| if s.is_empty() { None } else { s.parse::<u64>().ok() });

        let mut start = start_raw.unwrap_or(0);
        let mut end = match (start_raw, end_raw) {
            (None, Some(suffix)) => {
                start = (file_size.saturating_sub(suffix)).max(0);
                file_size.saturating_sub(1)
            }
            (_, Some(e)) => e,
            (_, None) => file_size.saturating_sub(1),
        };

        if start >= file_size || end < start {
            return (
                StatusCode::RANGE_NOT_SATISFIABLE,
                [(header::CONTENT_RANGE, format!("bytes */{file_size}"))],
                "",
            )
                .into_response();
        }
        if end >= file_size {
            end = file_size.saturating_sub(1);
        }
        let chunk_size = end - start + 1;

        let file = match tokio::fs::File::open(&path).await {
            Ok(f) => f,
            Err(_) => return err(StatusCode::INTERNAL_SERVER_ERROR, "cannot open file"),
        };
        let mut file2 = match file.try_clone().await {
            Ok(f) => f,
            Err(_) => file,
        };
        {
            use tokio::io::AsyncSeekExt;
            if file2.seek(std::io::SeekFrom::Start(start)).await.is_err() {
                return err(StatusCode::INTERNAL_SERVER_ERROR, "seek failed");
            }
        }
        let stream = tokio_util::io::ReaderStream::new(file2);

        return (
            StatusCode::PARTIAL_CONTENT,
            [
                (header::CONTENT_RANGE, format!("bytes {start}-{end}/{file_size}")),
                (header::ACCEPT_RANGES, "bytes".to_string()),
                (header::CONTENT_LENGTH, chunk_size.to_string()),
                (header::CONTENT_TYPE, mime.to_string()),
            ],
            axum::body::Body::from_stream(stream),
        )
            .into_response();
    }

    let file = match tokio::fs::File::open(&path).await {
        Ok(f) => f,
        Err(_) => return err(StatusCode::INTERNAL_SERVER_ERROR, "cannot open file"),
    };
    let stream = tokio_util::io::ReaderStream::new(file);
    (
        StatusCode::OK,
        [
            (header::CONTENT_LENGTH, file_size.to_string()),
            (header::CONTENT_TYPE, mime.to_string()),
            (header::ACCEPT_RANGES, "bytes".to_string()),
        ],
        axum::body::Body::from_stream(stream),
    )
        .into_response()
}

async fn cover_track(State(state): State<AppState>, AxumPath(track_id): AxumPath<i64>) -> Response {
    let Ok(conn) = get_conn(&state.pool) else {
        return err(StatusCode::INTERNAL_SERVER_ERROR, "db error");
    };

    // Prefer the requested track's OWN embedded cover first — this prevents
    // cross-contamination when multiple releases share the same album name +
    // album_artist (the scanner dedupes albums by those two columns only).
    // Only if the track itself has no cover do we fall back to a sibling track
    // in the same album (e.g. one file with art for the whole album).
    let cover_path: Option<String> = conn
        .query_row(
            "SELECT path FROM tracks WHERE id = ? AND has_cover = 1",
            [track_id],
            |row| row.get(0),
        )
        .ok()
        .or_else(|| {
            let album_id: Option<i64> = conn
                .query_row("SELECT album_id FROM tracks WHERE id = ?", [track_id], |row| row.get(0))
                .ok()
                .flatten();
            album_id.and_then(|aid| {
                conn.query_row(
                    "SELECT path FROM tracks WHERE album_id = ? AND has_cover = 1 ORDER BY id LIMIT 1",
                    [aid],
                    |row| row.get(0),
                )
                .ok()
                .or_else(|| {
                    conn.query_row(
                        "SELECT path FROM tracks WHERE album_id = ? ORDER BY id LIMIT 1",
                        [aid],
                        |row| row.get(0),
                    )
                    .ok()
                })
            })
        });

    let Some(cover_path) = cover_path else {
        return err(StatusCode::NOT_FOUND, "No cover");
    };
    match crate::metadata::extract_cover_from_path(std::path::Path::new(&cover_path)) {
        Ok((_, data, mime)) => {
            ([(header::CONTENT_TYPE, mime), (header::CACHE_CONTROL, "public, max-age=86400".to_string())], data).into_response()
        }
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, e),
    }
}

// ============================================================
// SCAN / SETTINGS / IMPORTS / ENRICHMENT ROUTES
// ============================================================

async fn scan_status(State(state): State<AppState>) -> Response {
    let Ok(conn) = get_conn(&state.pool) else {
        return err(StatusCode::INTERNAL_SERVER_ERROR, "db error");
    };
    let tracks: i64 = count_rows(&conn, "tracks");
    let albums: i64 = count_rows(&conn, "albums");
    let artists: i64 = count_rows(&conn, "artists");
    let playlists: i64 = count_rows(&conn, "playlists");
    Json(json!({ "tracks": tracks, "albums": albums, "artists": artists, "playlists": playlists })).into_response()
}

async fn rescan(State(state): State<AppState>) -> Response {
    let cfg = match load_config() {
        Ok(c) => c,
        Err(e) => return err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    let exts: Vec<std::sync::Arc<str>> = cfg.supported_extensions.iter().map(|s| s.as_str().into()).collect();
    match scan_library(&state.pool, &cfg.music_folder, &exts) {
        Ok(p) => Json(p).into_response(),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, e),
    }
}

async fn get_settings() -> Response {
    match load_config() {
        Ok(cfg) => Json(cfg).into_response(),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

async fn patch_settings(body: Json<Value>) -> Response {
    match patch_config(&body.0) {
        Ok(cfg) => Json(cfg).into_response(),
        Err(e) => err(StatusCode::BAD_REQUEST, e),
    }
}

#[derive(Deserialize)]
struct ValidateFolder { folder: String }

async fn validate_folder(Json(payload): Json<ValidateFolder>) -> Response {
    let valid = std::path::Path::new(&payload.folder).is_dir();
    let exists = std::fs::metadata(&payload.folder).map(|m| m.is_dir()).unwrap_or(false);
    Json(json!({ "valid": valid, "exists": exists })).into_response()
}

async fn import_m3u(State(state): State<AppState>, body: Json<Value>) -> Response {
    let file_path = body.0.get("filePath").and_then(|v| v.as_str()).unwrap_or("").trim();
    if file_path.is_empty() {
        return err(StatusCode::BAD_REQUEST, "filePath required");
    }
    let path = std::path::PathBuf::from(file_path);
    if !path.is_file() {
        return err(StatusCode::NOT_FOUND, format!("File not found: {}", path.display()));
    }
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
    if ext != "m3u" && ext != "m3u8" {
        return err(StatusCode::BAD_REQUEST, "Only .m3u/.m3u8 files are supported");
    }
    let Ok(mut conn) = get_conn(&state.pool) else {
        return err(StatusCode::INTERNAL_SERVER_ERROR, "db error");
    };
    let result = match crate::import_playlist::import_playlist_from_file(&mut conn, &path, true) {
        Ok(r) => r,
        Err(e) => return err(StatusCode::BAD_REQUEST, e),
    };
    let p = conn.query_row("SELECT id, name, description, created_at, is_imported, source FROM playlists WHERE id = ?", [result.playlist_id], |row| {
        Ok((
            row.get::<usize, i64>(0)?,
            row.get::<usize, String>(1)?,
            row.get::<usize, Option<String>>(2)?,
            row.get::<usize, String>(3)?,
            row.get::<usize, i64>(4)?,
            row.get::<usize, Option<String>>(5)?,
        ))
    });
    if let Ok((id, name, desc, created, imp, source)) = p {
        (StatusCode::CREATED, Json(json!({
            "playlist": shape_playlist_value(id, &name, desc.as_deref(), &created, imp == 1, source.as_deref(), Some(result.playlist_id), result.total_entries),
            "totalEntries": result.total_entries,
            "matched": result.matched,
            "missing": result.missing,
            "warnings": result.warnings,
        }))).into_response()
    } else {
        err(StatusCode::INTERNAL_SERVER_ERROR, "playlist gone")
    }
}

async fn get_lyrics(State(state): State<AppState>, AxumPath(track_id): AxumPath<i64>) -> Response {
    let pool = state.pool.clone();
    match crate::lyrics::get_lyrics_for_track(&pool, track_id).await {
        Some(payload) => {
            let synced = payload.synced && crate::lyrics::is_synced_lrc(&payload.lyrics);
            let lines = if synced { crate::lyrics::parse_lrc(&payload.lyrics) } else { Vec::new() };
            Json(crate::types::LyricsResult { lyrics: payload.lyrics, synced, lines }).into_response()
        }
        None => Json(crate::types::LyricsResult { lyrics: String::new(), synced: false, lines: Vec::new() }).into_response(),
    }
}

async fn get_features() -> Response {
    let cfg = match load_config() { Ok(c) => c, Err(_) => return err(StatusCode::INTERNAL_SERVER_ERROR, "config error") };
    Json(crate::types::Features {
        lyrics: cfg.enable_lyrics,
        canvas: cfg.enable_canvas && !cfg.sp_dc.is_empty(),
        has_sp_dc: !cfg.sp_dc.is_empty(),
        musixmatch_access_token: !cfg.musixmatch_access_token.is_empty(),
    }).into_response()
}

async fn get_canvas(State(state): State<AppState>, AxumPath(track_id): AxumPath<i64>) -> Response {
    let pool = state.pool.clone();
    match crate::canvas::get_canvas_for_track(&pool, track_id).await {
        Some(result) if !result.url.is_empty() => {
            Json(json!({
                "url": result.url,
                "artistUri": result.artist_uri,
                "artistName": result.artist_name,
                "artistImgUrl": result.artist_img_url,
            })).into_response()
        }
        _ => Json(json!({ "url": serde_json::Value::Null })).into_response(),
    }
}

#[derive(Deserialize)]
struct ClearCanvasBody {
    title: String,
    artist: String,
}

async fn clear_canvas_cache(State(state): State<AppState>, Json(body): Json<ClearCanvasBody>) -> Response {
    let pool = state.pool.clone();
    if body.title.trim().is_empty() || body.artist.trim().is_empty() {
        return err(StatusCode::BAD_REQUEST, "title and artist required");
    }
    match crate::canvas::clear_canvas_cache_by_title_artist(&pool, &body.title, &body.artist) {
        Ok(deleted) => Json(json!({ "deleted": deleted })).into_response(),
        Err(_) => err(StatusCode::INTERNAL_SERVER_ERROR, "db error"),
    }
}

async fn get_artist_image_route(State(state): State<AppState>, AxumPath(artist_id): AxumPath<i64>) -> Response {
    let pool = state.pool.clone();
    let Ok(conn) = get_conn(&pool) else {
        return err(StatusCode::INTERNAL_SERVER_ERROR, "db error");
    };
    let artist_name: Option<String> = conn
        .query_row("SELECT name FROM artists WHERE id = ?", [artist_id], |row| row.get(0))
        .ok();

    let Some(name) = artist_name else {
        return err(StatusCode::NOT_FOUND, "Artist not found");
    };

    let result = crate::artist_image::get_artist_image(&pool, &name, Some(artist_id)).await;
    Json(json!({
        "imageUrl": result.image_url,
        "spotifyUrl": result.spotify_url,
    })).into_response()
}

async fn get_track_artists(State(state): State<AppState>, AxumPath(track_id): AxumPath<i64>) -> Response {
    let Ok(conn) = get_conn(&state.pool) else {
        return err(StatusCode::INTERNAL_SERVER_ERROR, "db error");
    };
    let mut stmt = conn.prepare("SELECT a.id, a.name, a.image_path, ta.role FROM track_artists ta JOIN artists a ON a.id = ta.artist_id WHERE ta.track_id = ? ORDER BY ta.position").unwrap();
    let items: Vec<Value> = stmt.query_map([track_id], |row| {
        Ok(json!({
            "id": row.get::<usize, i64>(0)?,
            "name": row.get::<usize, String>(1)?,
            "imagePath": row.get::<usize, Option<String>>(2)?,
            "role": row.get::<usize, String>(3)?,
        }))
    }).unwrap().filter_map(|r| r.ok()).collect();
    Json(json!({ "items": items })).into_response()
}

async fn follow_artist(State(state): State<AppState>, AxumPath(artist_id): AxumPath<i64>, body: Json<Value>) -> Response {
    let follow = body.0.get("follow").and_then(|v| v.as_bool()).unwrap_or(false);
    let Ok(conn) = get_conn(&state.pool) else {
        return err(StatusCode::INTERNAL_SERVER_ERROR, "db error");
    };
    let exists = conn.query_row("SELECT 1 FROM artists WHERE id = ?", [artist_id], |_| Ok(true)).unwrap_or(false);
    if !exists {
        return err(StatusCode::NOT_FOUND, "Artist not found");
    }
    if follow {
        let _ = conn.execute("INSERT OR REPLACE INTO artist_follows (artist_id, followed_at) VALUES (?, ?)", params![artist_id, chrono::Utc::now().to_rfc3339()]);
    } else {
        let _ = conn.execute("DELETE FROM artist_follows WHERE artist_id = ?", [artist_id]);
    }
    Json(json!({ "artistId": artist_id, "follow": follow })).into_response()
}

async fn get_follows(State(state): State<AppState>) -> Response {
    let Ok(conn) = get_conn(&state.pool) else {
        return err(StatusCode::INTERNAL_SERVER_ERROR, "db error");
    };
    let mut stmt = conn.prepare("SELECT af.artist_id, af.followed_at, a.name FROM artist_follows af JOIN artists a ON a.id = af.artist_id ORDER BY af.followed_at DESC").unwrap();
    let items: Vec<Value> = stmt.query_map([], |row| {
        Ok(json!({
            "artistId": row.get::<usize, i64>(0)?,
            "followedAt": row.get::<usize, String>(1)?,
            "name": row.get::<usize, String>(2)?,
        }))
    }).unwrap().filter_map(|r| r.ok()).collect();
    Json(json!({ "items": items })).into_response()
}

// ============================================================
// ROUTER BUILDER
// ============================================================

pub fn build_router() -> Router<AppState> {
    Router::new()
        .route("/api/health", get(|| async { Json(json!({ "ok": true, "version": "0.1.0" })) }))
        .route("/api/library/tracks", get(get_tracks))
        .route("/api/library/search", get(search))
        .route("/api/library/albums", get(get_albums))
        .route("/api/library/albums/:id", get(get_album))
        .route("/api/library/artists", get(get_artists))
        .route("/api/library/artists/:id", get(get_artist))
        .route("/api/library/artists/:id/tracks", get(get_artist_tracks))
        .route("/api/library/liked", get(get_liked))
        .route("/api/library/liked/:track_id", post(toggle_like))
        .route("/api/library/played/:track_id", post(mark_played))
        .route("/api/library/tracks/:track_id/metadata", get(get_track_metadata).patch(patch_track_metadata))
        .route("/api/playlists", get(get_playlists).post(create_playlist))
        .route("/api/playlists/:id", get(get_playlist).patch(rename_playlist).delete(delete_playlist))
        .route("/api/playlists/:id/tracks", post(add_tracks_to_playlist))
        .route("/api/playlists/:id/tracks/:track_id", delete(remove_track_from_playlist))
        .route("/api/playlists/:id/reorder", post(reorder_playlist))
        .route("/api/playlists/reorder", post(reorder_playlists))
        .route("/api/stream/:track_id", get(stream_track))
        .route("/api/cover/:track_id", get(cover_track))
        .route("/api/scan/status", get(scan_status))
        .route("/api/scan/rescan", post(rescan))
        .route("/api/settings", get(get_settings).patch(patch_settings))
        .route("/api/settings/validate-folder", post(validate_folder))
        .route("/api/imports/import", post(import_m3u))
        .route("/api/lyrics/:track_id", get(get_lyrics))
        .route("/api/features", get(get_features))
        .route("/api/canvas/:track_id", get(get_canvas))
        .route("/api/canvas/clear-cache", post(clear_canvas_cache))
        .route("/api/artist-image/:artist_id", get(get_artist_image_route))
        .route("/api/tracks/:track_id/artists", get(get_track_artists))
        .route("/api/follows", get(get_follows))
        .route("/api/follows/:artist_id", post(follow_artist))
        .route("/api/smc/update", post(smc_update))
        .route("/api/smc/events", get(smc_events))
}

// ════════════════════════════════════════════════════════════
// SYSTEM MEDIA CONTROLS (SMTC)
// ════════════════════════════════════════════════════════════

async fn smc_update(body: Json<Value>) -> Response {
    let v = body.0;
    let state = crate::media_controls::SmcState {
        is_playing: v.get("isPlaying").and_then(|x| x.as_bool()).unwrap_or(false),
        title: v.get("title").and_then(|x| x.as_str()).unwrap_or("").to_string(),
        artist: v.get("artist").and_then(|x| x.as_str()).unwrap_or("").to_string(),
        album: v.get("album").and_then(|x| x.as_str()).unwrap_or("").to_string(),
        duration_secs: v.get("duration").and_then(|x| x.as_f64()).unwrap_or(0.0),
        position_secs: v.get("position").and_then(|x| x.as_f64()).unwrap_or(0.0),
    };
    crate::media_controls::update(state);
    Json(json!({ "ok": true })).into_response()
}

async fn smc_events() -> Response {
    let events = crate::media_controls::drain_events();
    let items: Vec<Value> = events
        .iter()
        .map(|s| serde_json::from_str(s).unwrap_or(Value::Null))
        .collect();
    Json(json!({ "events": items })).into_response()
}
