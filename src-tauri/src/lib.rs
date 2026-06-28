//! LocalWave Tauri backend.
//!
//! The Rust crate is split into a library (lib.rs) and a small binary (main.rs).
//! The library contains the embedded `axum` server, SQLite database, file
//! watcher, and all route handlers.

pub mod artist_image;
pub mod artist_parser;
pub mod canvas;
pub mod config;
pub mod db;
pub mod import_playlist;
pub mod lyrics;
pub mod metadata;
pub mod m3u_parser;
pub mod musixmatch;
pub mod proto_canvas;
pub mod routes;
pub mod scanner;
pub mod spotify_api_helper;
pub mod spotify_auth;
pub mod state;
pub mod types;
pub mod watcher;

use std::sync::Arc;

use crate::db::DbPool;

/// Application state shared across axum handlers.
#[derive(Clone)]
pub struct AppState {
    pub pool: Arc<DbPool>,
}
