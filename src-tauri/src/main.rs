//! LocalWave desktop app entry point.
//!
//! 1. Initialises logging, config, data directory, and SQLite pool.
//! 2. Starts the embedded axum HTTP server in a background tokio task.
//! 3. Performs the initial library scan, then starts the file watcher.
//! 4. Hands off to Tauri to run the webview + frontend.

use std::sync::Arc;

use tauri::Manager;

use localwave_lib::config::{ensure_data_dir, load_config};
use localwave_lib::db::init_pool;
use localwave_lib::routes::build_router;
use localwave_lib::scanner::scan_library;
use localwave_lib::spotify_auth::init_spotify_auth;
use localwave_lib::state::init_pool as init_state_pool;
use localwave_lib::watcher::start_watcher;
use localwave_lib::AppState;

const DEFAULT_HTTP_PORT: u16 = 8787;

fn main() {
    // Initialize env logger; RUST_LOG=info,localwave=debug in dev.
    env_logger::init();
    log::info!("[localwave] starting LocalWave Tauri app");

    let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
    let _guard = rt.enter();

    // 1. Config + data dir + DB pool
    ensure_data_dir().expect("failed to create data directory");
    let config = load_config().expect("failed to load config");
    let db_pool = init_pool(&localwave_lib::config::db_path()).expect("failed to initialize database");
    init_state_pool(db_pool.clone());

    // 2b. Initialize Spotify enrichment services (TOTP secrets + periodic refresh)
    rt.spawn(async move {
        init_spotify_auth().await;
    });
    let app_state = AppState {
        pool: Arc::new(db_pool.clone()),
    };
    let port = config.port;
    let cors = tower_http::cors::CorsLayer::new()
        .allow_origin(tower_http::cors::Any)
        .allow_methods(tower_http::cors::Any)
        .allow_headers(tower_http::cors::Any);
    let app = build_router().layer(cors).with_state(app_state.clone());

    let server_handle = rt.spawn(async move {
        let listener = match tokio::net::TcpListener::bind(format!("127.0.0.1:{port}")).await {
            Ok(l) => l,
            Err(e) => {
                log::error!("[localwave] failed to bind embedded HTTP server on port {port}: {e}");
                return;
            }
        };
        log::info!("[localwave] embedded API listening on http://127.0.0.1:{port}");
        if let Err(e) = axum::serve(listener, app).await {
            log::error!("[localwave] axum server error: {e}");
        }
    });

    // 3. Initial scan (non-blocking) + watcher
    let scan_pool = db_pool.clone();
    let music_folder = config.music_folder.clone();
    let exts: Vec<Arc<str>> = config
        .supported_extensions
        .iter()
        .map(|s| s.as_str().into())
        .collect();
    let watcher_exts = exts.clone();
    let watcher_folder = music_folder.clone();

    rt.spawn(async move {
        log::info!("[localwave] starting initial library scan...");
        match scan_library(&scan_pool, &music_folder, &exts) {
            Ok(p) => {
                log::info!(
                    "[localwave] scan done: {} scanned, {} added, {} updated, {} failed, {} playlists imported",
                    p.scanned, p.added, p.updated, p.failed, p.playlists_imported
                );
                if let Some(_watcher) = start_watcher(scan_pool, watcher_folder, watcher_exts) {
                    // watcher is kept alive by the returned handle
                    std::mem::forget(_watcher);
                }
            }
            Err(e) => log::error!("[localwave] initial scan failed: {e}"),
        }
    });

    // 4. Tauri app
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .on_window_event(move |_window, _event| {})
        .setup(move |_app| {
            // any app-specific setup can go here
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");

    // When Tauri exits, abort the server task.
    server_handle.abort();
}
