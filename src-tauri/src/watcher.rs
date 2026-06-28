//! Filesystem watcher. Port of `server/src/watcher/index.ts`.
//!
//! Uses `notify` + `notify-debouncer-full` to replace `chokidar`.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use notify_debouncer_full::{new_debouncer, DebounceEventResult, Debouncer, FileIdMap};
use rusqlite::params;

use crate::db::{get_conn, DbPool};
use crate::scanner::{remove_playlist_by_path, remove_track_by_path, scan_single_file, PLAYLIST_EXTS};

pub struct Watcher {
    _debouncer: Debouncer<notify::RecommendedWatcher, FileIdMap>,
}

fn is_audio(path: &Path, exts: &[Arc<str>]) -> bool {
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
    let ext_with_dot = format!(".{ext}");
    exts.iter().any(|e| e.eq_ignore_ascii_case(&ext_with_dot) || e.eq_ignore_ascii_case(&ext))
}

fn is_playlist(path: &Path) -> bool {
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
    PLAYLIST_EXTS.contains(&format!(".{ext}"))
}

fn is_supported(path: &Path, exts: &[Arc<str>]) -> bool {
    is_audio(path, exts) || is_playlist(path)
}

fn prune_tracks_under(pool: &DbPool, dir: &Path) {
    let Ok(conn) = get_conn(pool) else { return };
    let prefix = if dir.to_string_lossy().ends_with(std::path::MAIN_SEPARATOR) {
        dir.to_path_buf()
    } else {
        let mut d = dir.to_path_buf();
        d.push(""); // appends separator
        d
    };
    let prefix_lower = prefix.to_string_lossy().to_lowercase();

    let mut stmt = match conn.prepare("SELECT id, path FROM tracks") {
        Ok(s) => s,
        Err(_) => return,
    };
    let to_delete: Vec<i64> = stmt
        .query_map([], |row| {
            let id: i64 = row.get(0)?;
            let path: String = row.get(1)?;
            Ok((id, path))
        })
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|r| r.ok())
        .filter(|(_, p)| p.to_lowercase().starts_with(&prefix_lower))
        .map(|(id, _)| id)
        .collect();
    drop(stmt);

    if to_delete.is_empty() {
        return;
    }
    log::info!("[watcher] pruned {} tracks under {}", to_delete.len(), dir.display());
    for id in to_delete {
        let _ = conn.execute("DELETE FROM tracks WHERE id = ?", [id]);
    }
}

pub fn start_watcher(pool: DbPool, music_folder: String, supported_extensions: Vec<Arc<str>>) -> Option<Watcher> {
    let music_folder_path = PathBuf::from(&music_folder);
    if !music_folder_path.is_dir() {
        log::warn!("[watcher] music folder does not exist, not watching: {}", music_folder);
        return None;
    }

    let pool_inner = pool.clone();
    let exts = supported_extensions.clone();

    let mut debouncer = new_debouncer(
        Duration::from_millis(500),
        None,
        move |result: DebounceEventResult| {
            let events = match result {
                Ok(e) => e,
                Err(errs) => {
                    for e in errs {
                        log::error!("[watcher] error: {:?}", e);
                    }
                    return;
                }
            };

            for event in events {
                use notify::EventKind;
                for path in &event.paths {
                    match event.kind {
                        EventKind::Create(_) | EventKind::Modify(_) => {
                            if is_supported(path, &exts) {
                                log::info!("[watcher] {} detected: {}",
                                    if matches!(event.kind, EventKind::Create(_)) { "add" } else { "change" },
                                    path.display());
                                let _ = scan_single_file(&pool_inner, path);
                            }
                        }
                        EventKind::Remove(_) => {
                            if is_audio(path, &exts) {
                                log::info!("[watcher] unlink detected: {}", path.display());
                                remove_track_by_path(&pool_inner, &path.to_string_lossy());
                            } else if is_playlist(path) {
                                log::info!("[watcher] playlist unlink detected: {}", path.display());
                                remove_playlist_by_path(&pool_inner, &path.to_string_lossy());
                            }
                        }
                        _ => {}
                    }
                }
            }
        },
    )
    .ok()?;

    // watch recursively; filtering of unsupported files happens in the event
    // callback above (is_supported check) since notify-debouncer-full 0.4
    // `watch()` does not accept a filter closure.
    debouncer
        .watch(&music_folder_path, notify::RecursiveMode::Recursive)
        .ok()?;

    log::info!("[watcher] initial scan complete, watching for changes in {}", music_folder);
    Some(Watcher { _debouncer: debouncer })
}
