import { create } from 'zustand';
import type { Track, Album, Artist, Playlist, ScanStatus } from '../types';
import { api } from '../lib/api';

interface LibraryState {
  tracks: Track[];
  albums: Album[];
  artists: Artist[];
  playlists: Playlist[];
  scanStatus: ScanStatus | null;
  loading: boolean;
  error: string | null;

  loadAll: () => Promise<void>;
  loadTracks: (q?: string) => Promise<void>;
  loadAlbums: () => Promise<void>;
  loadArtists: () => Promise<void>;
  loadPlaylists: () => Promise<void>;
  refreshStatus: () => Promise<void>;
  toggleLikeOptimistic: (trackId: number) => void;
  setTracks: (t: Track[]) => void;
}

export const useLibraryStore = create<LibraryState>((set, get) => ({
  tracks: [],
  albums: [],
  artists: [],
  playlists: [],
  scanStatus: null,
  loading: false,
  error: null,

  loadAll: async () => {
    set({ loading: true, error: null });
    try {
      await Promise.all([
        get().loadTracks(),
        get().loadAlbums(),
        get().loadArtists(),
        get().loadPlaylists(),
        get().refreshStatus(),
      ]);
    } catch (err) {
      set({ error: (err as Error).message });
    } finally {
      set({ loading: false });
    }
  },

  loadTracks: async (q) => {
    try {
      const { items } = await api.getTracks(q);
      set({ tracks: items });
    } catch (err) {
      set({ error: (err as Error).message });
    }
  },

  loadAlbums: async () => {
    try {
      const { items } = await api.getAlbums();
      set({ albums: items });
    } catch (err) {
      set({ error: (err as Error).message });
    }
  },

  loadArtists: async () => {
    try {
      const { items } = await api.getArtists();
      set({ artists: items });
    } catch (err) {
      set({ error: (err as Error).message });
    }
  },

  loadPlaylists: async () => {
    try {
      const { items } = await api.getPlaylists();
      set({ playlists: items });
    } catch (err) {
      set({ error: (err as Error).message });
    }
  },

  refreshStatus: async () => {
    try {
      const status = await api.getScanStatus();
      set({ scanStatus: status });
    } catch {
      /* non-fatal */
    }
  },

  toggleLikeOptimistic: (trackId) => {
    set((s) => ({
      tracks: s.tracks.map((t) =>
        t.id === trackId ? { ...t, liked: !t.liked } : t,
      ),
    }));
  },

  setTracks: (t) => set({ tracks: t }),
}));
