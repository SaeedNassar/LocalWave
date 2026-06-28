# AGENTS.md

Local-first Spotify-style desktop music player. Tauri 2 shell (Rust + axum backend, React + Vite frontend) — there is **no separate Node server**. The Rust process embeds an axum HTTP API and serves the built React bundle inside the webview.

## Commands

```bash
npm install              # frontend deps
npm run dev              # Vite dev server on :1420 (frontend only, no backend)
npm run tauri dev        # full app: starts Rust backend + Vite + Tauri webview
npm run build            # production frontend build -> dist/
npm run tauri build      # production desktop bundle (.nsis / .msi)
npm run typecheck        # tsc -b --noEmit  — RUN BEFORE CLAIMING FRONTEND WORK DONE
```

There is no test suite, linter, or formatter configured — don't invent commands.

When touching Rust, `cargo check --manifest-path src-tauri/Cargo.toml` is the verification step (first build downloads and compiles many crates; expect minutes).

## Two-process architecture

- **Frontend** (`src/`): React 18 + react-router 6 + zustand 5 + Tailwind. Entry `src/main.tsx` → `App.tsx`. Path alias `@/*` → `src/*` (configured in both `tsconfig.json` and `vite.config.ts`).
- **Backend** (`src-tauri/src/`): Tauri 2 + tokio + axum + rusqlite (r2d2 pool) + `lofty` for tags + `notify` for file watching. Binary `main.rs` orchestrates startup; library crate `localwave_lib::*` holds all logic.

`src-tauri/src/main.rs` boot order matters and is non-obvious: `ensure_data_dir → load_config → init_pool → init_state_pool → spawn spotify_auth → build_router + axum::serve on :8787 → spawn scan_library → start_watcher → run Tauri`. The embedded HTTP server is **localhost:8787**, hard-coded as `API_ORIGIN` in `src/lib/api.ts` and referenced throughout `tauri.conf.json` CSP. Changing the port means editing three places (`config.rs` default, running config, frontend, CSP).

## Frontend ↔ backend contract

The frontend talks to the backend over plain `fetch` to `http://localhost:8787/api/*` — **not** via Tauri IPC commands. All routes are wired in `src-tauri/src/routes.rs::build_router`. Rust types in `types.rs` use `#[serde(rename_all = "camelCase")]` (and per-field `#[serde(rename = ...)]`) so JSON shapes match the TS interfaces in `src/types.ts` byte-for-byte. Editing one side means editing the other.

## Persistence & data locations

- Runtime data lives under `%APPDATA%/LocalWave/` (per-user, writable without elevation), **not** in the repo. Resolved in `src-tauri/src/config.rs::DATA_DIR`.
- `localwave.db` — SQLite (WAL mode), schema is migrated idempotently on every startup via `db.rs::migrate`. Add columns via `column_exists` guards, not destructive migrations.
- `config.json` — runtime config (`musicFolder`, `port`, `sp_dc`, feature flags). `load_config` memoizes; `patch_config` validates and atomic-writes (`.tmp` → rename). Corrupt config is backed up to `.broken` rather than overwritten.
- Player prefs (volume/mute/shuffle/repeat) persist via zustand `persist` to `localStorage` keyed `localwave-player`. Queue and currentTime are deliberately **not** persisted.

## Audio playback bridge — do not refactor naively

`src/hooks/usePlayer.ts` is a singleton-ish hook mounted once in `App.tsx`. It owns the hidden `<audio>` element and bridges it to `store/player.ts`. Two subtleties that are easy to break:

- **`pendingSeek`** — the store's `currentTime` is normally driven by the audio element's `timeupdate`, so store-only writes (`seek()`, `prev()` restart, `repeat='one'`) can't reach the element directly. They set `pendingSeek`, which the bridge effect applies to `audio.currentTime` and then clears. Don't "simplify" this away.
- **Play-count dedup** — `countedPlayRef` ensures a track is only counted once per load, so rapid skips don't inflate server-side play counts.
- **StrictMode double-mount** — cleanup pauses + releases the audio element to avoid orphaned playback. Preserve this.

Components subscribe to **specific** store fields, not the whole store (subscribing wholesale re-renders the app ~4×/sec via `timeupdate`).

## Styling

Tailwind config (`tailwind.config.ts`) defines the full LocalWave palette via semantic tokens (`bg-base`, `text-ink-base`, `brand`, `surface-*`, `edge-*`, `semantic-*`) and custom radii (`pill`, `pill-lg`), shadows (`elev`, `dialog`, `insetedge`), and font sizes (`micro`, `badge`). Prefer these tokens over raw hex. The app is dark-only (`<html class="dark">`).

## Enrichment features (Spotify/Musixmatch)

Lyrics, Canvas (looping video), and artist images pull from Spotify internals (`sp_dc` cookie) and Musixmatch. Auth lives in `spotify_auth.rs` (TOTP secret refresh); gating flags are `enable_lyrics` / `enable_canvas` in config and surfaced via `/api/features`. These are best-effort — don't treat missing enrichment as an error.

## Gotchas

- Vite dev port **1420** is fixed and `strictPort` (Tauri requires it). Don't change it.
- `vite.config.ts` sets `base: './'` so asset paths are relative and work under `tauri://localhost`. Don't switch to absolute paths.
- `noUnusedLocals`/`noUnusedParameters` are **off** — unused vars won't fail typecheck.
- `dist/` and `src-tauri/target/` are gitignored build outputs.
- `.tauri.conf.json` CSP allow-lists specific origins (`localhost:8787`, `*.scdn.co`, `lrclib.net`, `api.musixmatch.com`). New external endpoints must be added to `connect-src` / `media-src` / `img-src` or the webview will block them.
- File watching (`watcher.rs`) keeps its handle alive via `std::mem::forget` — dropping it stops the watcher.
