import type {
  Track,
  Album,
  Artist,
  ArtistDetail,
  ArtistTrack,
  TrackMetadata,
  TrackMetadataUpdate,
  ArtistImageResult,
  ArtistWithRole,
  CanvasResult,
  Features,
  LyricsResult,
  Playlist,
  PlaylistDetail,
  ScanStatus,
  ScanProgress,
  ImportResult,
  SearchResult,
} from '../types';

// The Rust backend (axum) is embedded in the Tauri app and listens here.
// In dev (Vite on :1420) and in the packaged app (tauri://localhost webview),
// the frontend talks to this absolute origin. CORS is enabled on the server.
export const API_ORIGIN = 'http://localhost:8787';
const BASE = `${API_ORIGIN}/api`;

async function http<T>(input: string, init?: RequestInit): Promise<T> {
  const res = await fetch(BASE + input, {
    ...init,
    headers: {
      'Content-Type': 'application/json',
      ...(init?.headers ?? {}),
    },
  });
  if (!res.ok) {
    let msg = `HTTP ${res.status}`;
    try {
      const body = await res.json();
      msg = body.error ?? msg;
    } catch {
      /* ignore */
    }
    throw new Error(msg);
  }
  return res.json() as Promise<T>;
}

export const api = {
  // library
  getTracks: (q?: string, limit = 500, offset = 0) => {
    const params = new URLSearchParams();
    if (q) params.set('q', q);
    params.set('limit', String(limit));
    params.set('offset', String(offset));
    return http<{ items: Track[]; total: number }>(`/library/tracks?${params}`);
  },
  getAlbums: () => http<{ items: Album[] }>(`/library/albums`),
  getAlbum: (id: number) =>
    http<{ album: Album; tracks: Track[] }>(`/library/albums/${id}`),
  getArtists: () => http<{ items: Artist[] }>(`/library/artists`),
  getArtist: (id: number) =>
    http<ArtistDetail>(`/library/artists/${id}`),
  getArtistTracks: (id: number) =>
    http<{ items: ArtistTrack[] }>(`/library/artists/${id}/tracks`),
  getTrackMetadata: (trackId: number) => http<TrackMetadata>(`/library/tracks/${trackId}/metadata`),
  updateTrackMetadata: (trackId: number, data: TrackMetadataUpdate) =>
    http<{ trackId: number; path: string }>(`/library/tracks/${trackId}/metadata`, {
      method: 'PATCH',
      body: JSON.stringify(data),
    }),
  search: (q: string) =>
    http<SearchResult>(`/library/search?q=${encodeURIComponent(q)}`),
  getLiked: () => http<{ items: Track[] }>(`/library/liked`),
  toggleLike: (trackId: number, liked: boolean) =>
    http<{ id: number; liked: boolean }>(`/library/liked/${trackId}`, {
      method: 'POST',
      body: JSON.stringify({ liked }),
    }),
  markPlayed: (trackId: number) =>
    http<{ ok: boolean }>(`/library/played/${trackId}`, { method: 'POST' }),

  // playlists
  getPlaylists: () => http<{ items: Playlist[] }>(`/playlists`),
  createPlaylist: (name: string, description?: string) =>
    http<Playlist>(`/playlists`, {
      method: 'POST',
      body: JSON.stringify({ name, description }),
    }),
  getPlaylist: (id: number) => http<PlaylistDetail>(`/playlists/${id}`),
  renamePlaylist: (id: number, name: string, description?: string) =>
    http<Playlist>(`/playlists/${id}`, {
      method: 'PATCH',
      body: JSON.stringify({ name, description }),
    }),
  deletePlaylist: (id: number) =>
    http<{ ok: boolean }>(`/playlists/${id}`, { method: 'DELETE' }),
  addTracksToPlaylist: (playlistId: number, trackIds: number[]) =>
    http<{ added: number }>(`/playlists/${playlistId}/tracks`, {
      method: 'POST',
      body: JSON.stringify({ trackIds }),
    }),
  removeTrackFromPlaylist: (playlistId: number, trackId: number) =>
    http<{ ok: boolean }>(`/playlists/${playlistId}/tracks/${trackId}`, {
      method: 'DELETE',
    }),
  reorderPlaylist: (playlistId: number, from: number, to: number) =>
    http<{ ok: boolean }>(`/playlists/${playlistId}/reorder`, {
      method: 'POST',
      body: JSON.stringify({ fromPosition: from, toPosition: to }),
    }),
  reorderPlaylists: (from: number, to: number) =>
    http<{ ok: boolean }>(`/playlists/reorder`, {
      method: 'POST',
      body: JSON.stringify({ fromPosition: from, toPosition: to }),
    }),

  // imports
  importM3u: (filePath: string) =>
    http<ImportResult>(`/imports/import`, {
      method: 'POST',
      body: JSON.stringify({ filePath }),
    }),

  // scan
  getScanStatus: () => http<ScanStatus>(`/scan/status`),
  rescan: () => http<ScanProgress>(`/scan/rescan`, { method: 'POST' }),

  // streaming / cover
  streamUrl: (trackId: number) => `${BASE}/stream/${trackId}`,
  coverUrl: (trackId: number) => `${BASE}/cover/${trackId}`,

  // enrichment
  getLyrics: (trackId: number) => http<LyricsResult>(`/lyrics/${trackId}`),
  getCanvas: (trackId: number) => http<CanvasResult>(`/canvas/${trackId}`),
  clearCanvasCache: (title: string, artist: string) =>
    http<{ deleted: number }>(`/canvas/clear-cache`, {
      method: 'POST',
      body: JSON.stringify({ title, artist }),
    }),
  getArtistImage: (artistId: number) =>
    http<ArtistImageResult>(`/artist-image/${artistId}`),
  getTrackArtists: (trackId: number) =>
    http<{ items: ArtistWithRole[] }>(`/tracks/${trackId}/artists`),
  getFeatures: () => http<Features>(`/features`),
  artistImageUrl: (artistId: number) => `${BASE}/artist-image/${artistId}`,
};
