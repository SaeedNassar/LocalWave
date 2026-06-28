//! Database layer: connection pool, idempotent migrations, backfill, and
//! row-shaping helpers. Mirrors `server/src/db.ts`.

use std::path::Path;
use std::sync::Arc;

use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{params, Connection, Row};

use crate::artist_parser::{normalize_artist_name, split_artists};
use crate::types::{Track, TrackArtist};

pub type DbPool = Pool<SqliteConnectionManager>;

/// Get a pooled connection. Used by route closures.
pub fn get_conn(pool: &DbPool) -> Result<PooledConnection<SqliteConnectionManager>, DbError> {
    pool.get().map_err(|e| DbError::Pool(e.to_string()))
}

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("pool error: {0}")]
    Pool(String),
    #[error("rusqlite: {0}")]
    Rusqlite(#[from] rusqlite::Error),
    #[error("{0}")]
    Other(String),
}

pub fn init_pool(db_path: &Path) -> Result<DbPool, DbError> {
    let manager = SqliteConnectionManager::file(db_path);
    let pool = Pool::builder()
        .max_size(5)
        .build(manager)
        .map_err(|e| DbError::Pool(e.to_string()))?;
    {
        let mut conn = get_conn(&pool)?;
        conn.execute_batch("PRAGMA journal_mode = WAL;")?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        migrate(&mut conn)?;
        backfill_artists(&mut conn)?;
    }
    Ok(pool)
}

fn column_exists(conn: &Connection, table: &str, column: &str) -> Result<bool, DbError> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", table))?;
    let rows = stmt.query_map([], |row| {
        let name: String = row.get("name")?;
        Ok(name)
    })?;
    for name in rows {
        if name? == column {
            return Ok(true);
        }
    }
    Ok(false)
}

fn table_exists(conn: &Connection, table: &str) -> Result<bool, DbError> {
    let mut stmt = conn.prepare(
    "SELECT name FROM sqlite_master WHERE type='table' AND name=?")?;
    let exists = stmt.exists([table])?;
    Ok(exists)
}

fn migrate(conn: &mut Connection) -> Result<(), DbError> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS artists (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE
        );

        CREATE TABLE IF NOT EXISTS albums (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            album_artist TEXT,
            artist_id INTEGER,
            year INTEGER,
            has_cover INTEGER NOT NULL DEFAULT 0,
            UNIQUE(name, album_artist),
            FOREIGN KEY (artist_id) REFERENCES artists(id) ON DELETE SET NULL
        );

        CREATE TABLE IF NOT EXISTS tracks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            path TEXT NOT NULL UNIQUE,
            filename TEXT NOT NULL,
            title TEXT NOT NULL,
            artist TEXT,
            album TEXT,
            album_artist TEXT,
            album_id INTEGER,
            artist_id INTEGER,
            duration REAL NOT NULL DEFAULT 0,
            track_number INTEGER,
            disk_number INTEGER,
            format TEXT,
            bitrate INTEGER,
            sample_rate INTEGER,
            has_cover INTEGER NOT NULL DEFAULT 0,
            liked INTEGER NOT NULL DEFAULT 0,
            play_count INTEGER NOT NULL DEFAULT 0,
            date_added TEXT NOT NULL,
            file_modified_at TEXT,
            FOREIGN KEY (album_id) REFERENCES albums(id) ON DELETE SET NULL,
            FOREIGN KEY (artist_id) REFERENCES artists(id) ON DELETE SET NULL
        );

        CREATE INDEX IF NOT EXISTS idx_tracks_album ON tracks(album_id);
        CREATE INDEX IF NOT EXISTS idx_tracks_artist ON tracks(artist_id);
        CREATE INDEX IF NOT EXISTS idx_tracks_liked ON tracks(liked);
        CREATE INDEX IF NOT EXISTS idx_tracks_title ON tracks(title);

        CREATE TABLE IF NOT EXISTS playlists (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            description TEXT,
            created_at TEXT NOT NULL,
            is_imported INTEGER NOT NULL DEFAULT 0,
            source TEXT
        );

        CREATE TABLE IF NOT EXISTS playlist_entries (
            playlist_id INTEGER NOT NULL,
            position INTEGER NOT NULL,
            track_id INTEGER,
            raw_entry TEXT NOT NULL,
            missing INTEGER NOT NULL DEFAULT 0,
            title TEXT,
            FOREIGN KEY (playlist_id) REFERENCES playlists(id) ON DELETE CASCADE,
            FOREIGN KEY (track_id) REFERENCES tracks(id) ON DELETE SET NULL,
            PRIMARY KEY (playlist_id, position)
        );

        CREATE INDEX IF NOT EXISTS idx_entries_playlist ON playlist_entries(playlist_id);
        "#,
    )?;

    // artists v2 columns
    if !column_exists(conn, "artists", "normalized_name")? {
        conn.execute("ALTER TABLE artists ADD COLUMN normalized_name TEXT", [])?;
    }
    if !column_exists(conn, "artists", "image_path")? {
        conn.execute("ALTER TABLE artists ADD COLUMN image_path TEXT", [])?;
    }
    if !column_exists(conn, "artists", "created_at")? {
        conn.execute("ALTER TABLE artists ADD COLUMN created_at TEXT", [])?;
    }

    conn.execute_batch(
        r#"
        CREATE UNIQUE INDEX IF NOT EXISTS idx_artists_normalized
            ON artists(normalized_name) WHERE normalized_name IS NOT NULL;

        CREATE TABLE IF NOT EXISTS track_artists (
            track_id INTEGER NOT NULL,
            artist_id INTEGER NOT NULL,
            role TEXT NOT NULL DEFAULT 'primary',
            position INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY (track_id) REFERENCES tracks(id) ON DELETE CASCADE,
            FOREIGN KEY (artist_id) REFERENCES artists(id) ON DELETE CASCADE,
            PRIMARY KEY (track_id, artist_id)
        );

        CREATE INDEX IF NOT EXISTS idx_track_artists_artist ON track_artists(artist_id);
        CREATE INDEX IF NOT EXISTS idx_track_artists_track ON track_artists(track_id);
        "#,
    )?;

    if !column_exists(conn, "playlists", "position")? {
        conn.execute("ALTER TABLE playlists ADD COLUMN position INTEGER", [])?;
    }

    // schema_flags + playlist position backfill
    conn.execute(
        "CREATE TABLE IF NOT EXISTS schema_flags (key TEXT PRIMARY KEY, value TEXT NOT NULL)",
        [],
    )?;

    let needs_backfill: Option<String> = conn
        .query_row(
            "SELECT value FROM schema_flags WHERE key = ?",
            ["playlists_position_backfilled"],
            |row| row.get(0),
        )
        .ok();
    if needs_backfill.is_none() {
        let mut stmt = conn.prepare("SELECT id FROM playlists ORDER BY created_at ASC, id ASC")?;
        let ids: Vec<i64> = stmt
            .query_map([], |row| row.get::<usize, i64>(0))?
            .collect::<Result<_, _>>()?;
        drop(stmt);
        let tx = conn.transaction()?;
        {
            let mut upd = tx.prepare("UPDATE playlists SET position = ? WHERE id = ?")?;
            for (i, id) in ids.iter().enumerate() {
                upd.execute(params![i as i64, id])?;
            }
        }
        tx.execute(
            "INSERT OR REPLACE INTO schema_flags (key, value) VALUES (?, ?)",
            ["playlists_position_backfilled", "1"],
        )?;
        tx.commit()?;
    }

    // enrichment tables
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS lyrics_cache (
            track_id INTEGER PRIMARY KEY,
            lyrics TEXT NOT NULL DEFAULT '',
            synced INTEGER NOT NULL DEFAULT 0,
            fetched_at TEXT NOT NULL,
            FOREIGN KEY (track_id) REFERENCES tracks(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS canvas_url_cache (
            track_id INTEGER PRIMARY KEY,
            url TEXT NOT NULL,
            artist_uri TEXT,
            artist_name TEXT,
            artist_img_url TEXT,
            fetched_at TEXT NOT NULL,
            FOREIGN KEY (track_id) REFERENCES tracks(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS artist_image_cache (
            artist_name TEXT PRIMARY KEY,
            spotify_url TEXT,
            image_url TEXT,
            fetched_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS artist_follows (
            artist_id INTEGER PRIMARY KEY,
            followed_at TEXT NOT NULL,
            FOREIGN KEY (artist_id) REFERENCES artists(id) ON DELETE CASCADE
        );
        "#,
    )?;

    Ok(())
}

fn backfill_artists(conn: &mut Connection) -> Result<(), DbError> {
    let flag: Option<String> = conn
        .query_row(
            "SELECT value FROM schema_flags WHERE key = ?",
            ["artists_backfilled"],
            |row| row.get(0),
        )
        .ok();
    if flag.as_deref() == Some("1") {
        return Ok(());
    }

    let count: i64 = conn.query_row("SELECT COUNT(*) FROM tracks", [], |row| row.get(0))?;
    if count == 0 {
        conn.execute(
            "INSERT OR REPLACE INTO schema_flags (key, value) VALUES (?, ?)",
            ["artists_backfilled", "1"],
        )?;
        return Ok(());
    }

    log::info!("[db] backfilling artists from existing tracks...");
    let mut stmt = conn.prepare("SELECT id, artist FROM tracks")?;
    let tracks: Vec<(i64, Option<String>)> = stmt
        .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?)))?
        .collect::<Result<_, _>>()?;
    drop(stmt);

    let tx = conn.transaction()?;
    {
        let mut upsert_artist = tx.prepare(
            "INSERT INTO artists (name, normalized_name, created_at) VALUES (?, ?, ?)
             ON CONFLICT(normalized_name) DO UPDATE SET name = CASE WHEN excluded.name != '' THEN excluded.name ELSE artists.name END
             ON CONFLICT(name) DO UPDATE SET normalized_name = COALESCE(artists.normalized_name, excluded.normalized_name)")?;
        let mut get_by_norm = tx.prepare("SELECT id FROM artists WHERE normalized_name = ?")?;
        let mut get_by_name = tx.prepare("SELECT id FROM artists WHERE name = ?")?;
        let mut clear_track_artists = tx.prepare("DELETE FROM track_artists WHERE track_id = ?")?;
        let mut insert_ta = tx.prepare(
            "INSERT OR REPLACE INTO track_artists (track_id, artist_id, role, position) VALUES (?, ?, ?, ?)")?;
        let mut update_track_primary = tx.prepare("UPDATE tracks SET artist_id = ? WHERE id = ?")?;
        let mut set_norm = tx.prepare("UPDATE artists SET normalized_name = ? WHERE id = ? AND normalized_name IS NULL")?;

        for (track_id, raw_artist) in tracks {
            let parsed = split_artists(raw_artist.as_deref());
            if parsed.is_empty() {
                let unknown = normalize_artist_name("Unknown Artist");
                let id: i64 = match get_by_norm.query_row([&unknown], |row| row.get(0)) {
                    Ok(id) => id,
                    Err(_) => {
                        upsert_artist.execute([
                            "Unknown Artist",
                            &unknown,
                            &chrono::Utc::now().to_rfc3339(),
                        ])?;
                        get_by_norm.query_row([&unknown], |row| row.get(0))?
                    }
                };
                clear_track_artists.execute([track_id])?;
                insert_ta.execute(params![track_id, id, "primary", 0])?;
                update_track_primary.execute(params![id, track_id])?;
                continue;
            }

            clear_track_artists.execute([track_id])?;
            let mut primary_id: Option<i64> = None;
            for pa in &parsed {
                let mut id: i64 = match get_by_norm.query_row([&pa.normalized_name], |row| row.get::<_, i64>(0)) {
                    Ok(id) => {
                        set_norm.execute([&pa.normalized_name, &id.to_string()])?;
                        id
                    }
                    Err(_) => match get_by_name.query_row([&pa.name], |row| row.get::<_, i64>(0)) {
                        Ok(id) => {
                            set_norm.execute([&pa.normalized_name, &id.to_string()])?;
                            id
                        }
                        Err(_) => {
                            upsert_artist.execute([
                                &pa.name,
                                &pa.normalized_name,
                                &chrono::Utc::now().to_rfc3339(),
                            ])?;
                            match get_by_norm.query_row([&pa.normalized_name], |row| row.get(0)) {
                                Ok(id) => id,
                                Err(_) => get_by_name.query_row([&pa.name], |row| row.get(0))?,
                            }
                        }
                    },
                };
                insert_ta.execute(params![track_id, id, &pa.role, pa.position as i64])?;
                if pa.role == "primary" && primary_id.is_none() {
                    primary_id = Some(id);
                }
            }
            if let Some(primary) = parsed.iter().find(|p| p.role == "primary").or_else(|| parsed.first()) {
                if let Ok(id) = get_by_norm.query_row([&primary.normalized_name], |row| row.get::<_, i64>(0)) {
                    update_track_primary.execute(params![id, track_id])?;
                }
            }
        }
    }
    tx.commit()?;

    conn.execute(
        "INSERT OR REPLACE INTO schema_flags (key, value) VALUES (?, ?)",
        ["artists_backfilled", "1"],
    )?;
    log::info!("[db] backfill complete: processed {} tracks", count);
    Ok(())
}

// ── row shaping ───────────────────────────────────────────────

fn bool_cell(v: Option<i64>) -> bool {
    v == Some(1)
}

pub fn row_to_track(row: &Row) -> Result<Track, rusqlite::Error> {
    Ok(Track {
        id: row.get("id")?,
        path: row.get("path")?,
        filename: row.get("filename")?,
        title: row.get("title")?,
        artist: row.get("artist")?,
        album: row.get("album")?,
        album_artist: row.get("album_artist")?,
        album_id: row.get("album_id")?,
        artist_id: row.get("artist_id")?,
        duration: row.get("duration")?,
        track_number: row.get("track_number")?,
        disk_number: row.get("disk_number")?,
        format: row.get("format")?,
        bitrate: row.get("bitrate")?,
        sample_rate: row.get("sample_rate")?,
        has_cover: bool_cell(row.get::<_, Option<i64>>("has_cover")?),
        liked: bool_cell(row.get::<_, Option<i64>>("liked")?),
        play_count: row.get("play_count")?,
        date_added: row.get("date_added")?,
        file_modified_at: row.get("file_modified_at")?,
        artists: Vec::new(),
    })
}

pub fn attach_track_artists(conn: &Connection, tracks: &mut [Track]) -> Result<(), rusqlite::Error> {
    if tracks.is_empty() {
        return Ok(());
    }
    let ids: Vec<i64> = tracks.iter().map(|t| t.id).collect();
    let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!(
        "SELECT ta.track_id AS track_id, a.id, a.name, ta.role, ta.position
         FROM track_artists ta
         JOIN artists a ON a.id = ta.artist_id
         WHERE ta.track_id IN ({})
         ORDER BY ta.track_id, ta.position",
        placeholders
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(&ids), |row| {
        Ok((
            row.get::<_, i64>("track_id")?,
            TrackArtist {
                id: row.get(1)?,
                name: row.get(2)?,
                role: row.get(3)?,
                position: row.get(4)?,
            },
        ))
    })?;

    let mut map: std::collections::HashMap<i64, Vec<TrackArtist>> = std::collections::HashMap::new();
    for row in rows {
        let (tid, artist) = row?;
        map.entry(tid).or_default().push(artist);
    }

    for track in tracks {
        track.artists = map.remove(&track.id).unwrap_or_default();
    }
    Ok(())
}
