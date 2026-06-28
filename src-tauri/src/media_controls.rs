//! Windows System Media Transport Controls (SMTC) integration.
//!
//! Runs a dedicated thread that owns the `MediaControls` instance (souvlaki
//! requires its handle to stay on the thread that created it). The frontend
//! pushes playback state over an MPSC sender; OS button-press events are
//! collected into a channel that the frontend polls via HTTP.

use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Mutex;
use std::time::Duration;

use once_cell::sync::Lazy;
use souvlaki::{MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig};

#[derive(Debug, Clone)]
pub struct SmcState {
    pub is_playing: bool,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_secs: f64,
    pub position_secs: f64,
}

impl Default for SmcState {
    fn default() -> Self {
        Self {
            is_playing: false,
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            duration_secs: 0.0,
            position_secs: 0.0,
        }
    }
}

/// Commands sent from the frontend (via HTTP) to the media-controls thread.
pub enum SmcCommand {
    Update(SmcState),
    Shutdown,
}

pub struct MediaControlsHandle {
    pub tx: Sender<SmcCommand>,
    pub events_rx: Receiver<MediaControlEvent>,
}

// ── Global singleton ─────────────────────────────────────────
// The media-controls thread is started once at boot and shared by all
// axum route handlers. The command sender is cheap to clone; the event
// receiver is drained by the polling route.

static SMC_HANDLE: Lazy<Mutex<Option<MediaControlsHandle>>> = Lazy::new(|| Mutex::new(None));

/// Initialize the global media-controls singleton. Called once from Tauri's
/// `.setup()` callback with the main window's HWND.
/// On non-Windows platforms, `hwnd` should be `None`.
pub fn init(hwnd: Option<usize>) {
    if let Some(h) = spawn_media_controls(hwnd) {
        *SMC_HANDLE.lock().unwrap() = Some(h);
    }
}

/// Push a state update to the media-controls thread (non-blocking, best-effort).
pub fn update(state: SmcState) {
    if let Some(handle) = SMC_HANDLE.lock().unwrap().as_ref() {
        let _ = handle.tx.send(SmcCommand::Update(state));
    }
}

/// Drain all pending OS media-button events. Called by the polling route.
pub fn drain_events() -> Vec<MediaControlEvent> {
    let mut events = Vec::new();
    if let Some(handle) = SMC_HANDLE.lock().unwrap().as_ref() {
        while let Ok(event) = handle.events_rx.try_recv() {
            events.push(event);
        }
    }
    events
}

// ── Thread + souvlaki ────────────────────────────────────────

/// Spawn the media-controls thread. Returns a handle for sending state updates
/// and receiving OS media-button events. Best-effort — logs and returns None
/// if souvlaki fails to init (e.g. unsupported platform).
///
/// `hwnd` is the raw window handle (Windows only) — required by souvlaki for
/// SMTC to function correctly. Without it, `set_metadata` silently fails.
struct SendPtr(*mut std::ffi::c_void);
unsafe impl Send for SendPtr {}

fn spawn_media_controls(hwnd: Option<usize>) -> Option<MediaControlsHandle> {
    let (cmd_tx, cmd_rx) = mpsc::channel::<SmcCommand>();
    let (evt_tx, evt_rx) = mpsc::channel::<MediaControlEvent>();

    let hwnd_ptr = hwnd.map(|h| SendPtr(h as *mut std::ffi::c_void));

    let init_result = std::thread::Builder::new()
        .name("localwave-smtc".into())
        .spawn(move || run_loop(cmd_rx, evt_tx, hwnd_ptr));

    if init_result.is_err() {
        log::warn!("[smtc] failed to spawn media-controls thread");
        return None;
    }

    log::info!("[smtc] media controls thread started (hwnd={})", hwnd.is_some());
    Some(MediaControlsHandle {
        tx: cmd_tx,
        events_rx: evt_rx,
    })
}

fn run_loop(cmd_rx: Receiver<SmcCommand>, evt_tx: Sender<MediaControlEvent>, hwnd: Option<SendPtr>) {
    // On Windows, souvlaki's SMTC backend uses COM. We must initialize COM on
    // this thread before creating the MediaControls instance.
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
        let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    }

    let config = PlatformConfig {
        dbus_name: "localwave",
        display_name: "LocalWave",
        hwnd: hwnd.map(|h| h.0),
    };

    let mut controls = match MediaControls::new(config) {
        Ok(c) => c,
        Err(e) => {
            log::warn!("[smtc] failed to create MediaControls: {:?}", e);
            return;
        }
    };

    let tx_clone = evt_tx.clone();
    if let Err(e) = controls.attach(move |event: MediaControlEvent| {
        let _ = tx_clone.send(event);
    }) {
        log::warn!("[smtc] failed to attach event handler: {:?}", e);
        return;
    }

    log::info!("[smtc] attached — listening for OS media button events");
    let _ = controls.set_playback(MediaPlayback::Stopped);

    let mut current = SmcState::default();

    loop {
        loop {
            match cmd_rx.try_recv() {
                Ok(SmcCommand::Update(state)) => apply_state(&mut controls, &mut current, &state),
                Ok(SmcCommand::Shutdown) => {
                    log::info!("[smtc] shutdown received — stopping");
                    let _ = controls.set_playback(MediaPlayback::Stopped);
                    return;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    log::info!("[smtc] command channel disconnected — stopping");
                    return;
                }
            }
        }

        #[cfg(target_os = "windows")]
        pump_windows_messages();

        std::thread::sleep(Duration::from_millis(50));
    }
}

fn apply_state(controls: &mut MediaControls, current: &mut SmcState, state: &SmcState) {
    let meta_changed = state.title != current.title
        || state.artist != current.artist
        || state.album != current.album
        || state.duration_secs != current.duration_secs;

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

    let playback_changed = state.is_playing != current.is_playing
        || (state.is_playing && (state.position_secs - current.position_secs).abs() > 1.0);

    if playback_changed {
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

    *current = state.clone();
}

#[cfg(target_os = "windows")]
fn pump_windows_messages() {
    use windows::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE,
    };

    unsafe {
        let mut msg = MSG::default();
        while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).into() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
