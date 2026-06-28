//! Windows System Media Transport Controls (SMTC) integration.
//!
//! The `MediaControls` instance MUST live on the main thread — the same
//! thread that runs the Tauri / Windows message loop. A background thread
//! creates a *different* SMTC session that Windows doesn't display as the
//! default one.
//!
//! Architecture:
//!  - `init()` is called from Tauri's `.setup()` callback (main thread).
//!  - It creates `MediaControls` in a `thread_local!` on the main thread.
//!  - Route handlers push state via `update()`, which dispatches work to
//!    the main thread using `AppHandle::run_on_main_thread()`.
//!  - OS media-button events are buffered in a global `Mutex<Vec>` that
//!    the frontend polls via HTTP.

use std::cell::RefCell;
use std::sync::Mutex;
use std::time::Duration;

use once_cell::sync::Lazy;
use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
};

#[derive(Debug, Clone, Default)]
pub struct SmcState {
    pub is_playing: bool,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_secs: f64,
    pub position_secs: f64,
}

// ── thread_local: lives only on the main thread ──────────────

thread_local! {
    static CONTROLS: RefCell<Option<MediaControls>> = const { RefCell::new(None) };
    static CURRENT_STATE: RefCell<SmcState> = RefCell::new(SmcState::default());
}

// ── Globals accessible from any thread ───────────────────────

/// Buffered OS media-button events (filled from the souvlaki callback).
static EVENTS: Lazy<Mutex<Vec<MediaControlEvent>>> = Lazy::new(|| Mutex::new(Vec::new()));

/// Tauri AppHandle — used to dispatch `set_metadata` / `set_playback` calls
/// onto the main thread where the `MediaControls` lives.
static APP_HANDLE: Lazy<Mutex<Option<tauri::AppHandle>>> = Lazy::new(|| Mutex::new(None));

// ── Init (call from Tauri .setup() on the main thread) ──────

pub fn init(app: tauri::AppHandle, hwnd: Option<usize>) {
    let hwnd_ptr = hwnd.map(|h| h as *mut std::ffi::c_void);

    let config = PlatformConfig {
        dbus_name: "localwave",
        display_name: "LocalWave",
        hwnd: hwnd_ptr,
    };

    match MediaControls::new(config) {
        Ok(mut controls) => {
            let _ = controls.attach(|event: MediaControlEvent| {
                if let Ok(mut q) = EVENTS.lock() {
                    q.push(event);
                }
            });
            CONTROLS.with(|c| {
                *c.borrow_mut() = Some(controls);
            });
            log::info!("[smtc] MediaControls created on main thread");
        }
        Err(e) => {
            log::warn!("[smtc] failed to create MediaControls: {:?}", e);
        }
    }

    *APP_HANDLE.lock().unwrap() = Some(app);
}

// ── Push state from route handlers (any thread) ──────────────

/// Push a state update. Dispatches the actual `set_metadata` / `set_playback`
/// calls to the main thread via `run_on_main_thread`, because `MediaControls`
/// lives in a thread_local there.
pub fn update(state: SmcState) {
    let app = APP_HANDLE.lock().unwrap().clone();
    let Some(app) = app else { return };

    let _ = app.run_on_main_thread(move || {
        CONTROLS.with(|cell| {
            let mut controls_ref = cell.borrow_mut();
            let Some(controls) = controls_ref.as_mut() else { return };

            let prev = CURRENT_STATE.with(|c| c.borrow().clone());
            let meta_changed = state.title != prev.title
                || state.artist != prev.artist
                || state.album != prev.album
                || state.duration_secs != prev.duration_secs;

            let play_changed = state.is_playing != prev.is_playing;
            let seeked = (state.position_secs - prev.position_secs).abs() > 1.5;

            // Push metadata when track changes.
            if meta_changed {
                if let Err(e) = controls.set_metadata(MediaMetadata {
                    title: if state.title.is_empty() { None } else { Some(&state.title) },
                    album: if state.album.is_empty() { None } else { Some(&state.album) },
                    artist: if state.artist.is_empty() { None } else { Some(&state.artist) },
                    cover_url: None,
                    duration: if state.duration_secs > 0.0 {
                        Some(Duration::from_secs_f64(state.duration_secs))
                    } else {
                        None
                    },
                }) {
                    log::warn!("[smtc] set_metadata FAILED: {:?}", e);
                }
            }

            // Push playback + timeline on track change, play/pause, or seek.
            if meta_changed || play_changed || seeked {
                let progress = if state.duration_secs > 0.0 {
                    Some(MediaPosition(Duration::from_secs_f64(state.position_secs)))
                } else {
                    None
                };
                let playback = if state.is_playing {
                    MediaPlayback::Playing { progress }
                } else {
                    MediaPlayback::Paused { progress }
                };
                if let Err(e) = controls.set_playback(playback) {
                    log::warn!("[smtc] set_playback FAILED: {:?}", e);
                }
            }

            CURRENT_STATE.with(|c| *c.borrow_mut() = state);
        });
    });
}

// ── Drain events for the frontend poll route ─────────────────

pub fn drain_events() -> Vec<String> {
    let mut events = Vec::new();
    if let Ok(mut q) = EVENTS.lock() {
        for event in q.drain(..) {
            events.push(event_to_json(&event));
        }
    }
    events
}

fn event_to_json(event: &MediaControlEvent) -> String {
    match event {
        MediaControlEvent::Play => r#"{"type":"play"}"#.into(),
        MediaControlEvent::Pause => r#"{"type":"pause"}"#.into(),
        MediaControlEvent::Toggle => r#"{"type":"toggle"}"#.into(),
        MediaControlEvent::Next => r#"{"type":"next"}"#.into(),
        MediaControlEvent::Previous => r#"{"type":"prev"}"#.into(),
        MediaControlEvent::Stop => r#"{"type":"stop"}"#.into(),
        MediaControlEvent::SetPosition(pos) => {
            format!(r#"{{"type":"seek","position":{}}}"#, pos.0.as_secs_f64())
        }
        _ => format!(r#"{{"type":"{:?}"}}"#, event),
    }
}
