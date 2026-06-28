import { create } from 'zustand';
import { persist, createJSONStorage } from 'zustand/middleware';
import type { Track } from '../types';
import { useUIStore } from './ui';

export type RepeatMode = 'off' | 'all' | 'one';

interface PlayerState {
  // queue
  queue: Track[];
  currentIndex: number;
  // ui
  isPlaying: boolean;
  currentTime: number;
  duration: number;
  volume: number;
  muted: boolean;
  shuffle: boolean;
  repeat: RepeatMode;
  // seek intent consumed by the audio bridge (store -> HTMLAudioElement).
  // The store's currentTime is normally driven by the audio's timeupdate, so
  // store-only writes from seek()/prev()/repeat-one can't move the element.
  // Setting pendingSeek makes the bridge apply it to audio.currentTime.
  pendingSeek: number | null;
  // actions
  playTrack: (track: Track, queue?: Track[]) => void;
  playQueue: (tracks: Track[], startIndex?: number) => void;
  togglePlay: () => void;
  next: () => void;
  prev: () => void;
  seek: (time: number) => void;
  setCurrentTime: (time: number) => void;
  setDuration: (duration: number) => void;
  setVolume: (v: number) => void;
  toggleMute: () => void;
  toggleShuffle: () => void;
  cycleRepeat: () => void;
  addToQueue: (track: Track) => void;
  removeFromQueue: (index: number) => void;
}

function shuffleArray<T>(arr: T[]): T[] {
  const a = [...arr];
  for (let i = a.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [a[i], a[j]] = [a[j], a[i]];
  }
  return a;
}

export const usePlayerStore = create<PlayerState>()(
  persist(
    (set, get) => ({
      queue: [],
      currentIndex: 0,
      isPlaying: false,
      currentTime: 0,
      duration: 0,
      volume: 0.8,
      muted: false,
      shuffle: false,
      repeat: 'off',
      pendingSeek: null,

      playTrack: (track, queue) => {
        const q = queue ?? [track];
        const idx = Math.max(0, q.findIndex((t) => t.id === track.id));
        set({ queue: q, currentIndex: idx, isPlaying: true, currentTime: 0, pendingSeek: null });
        useUIStore.getState().autoOpenPanelOnce();
      },

      playQueue: (tracks, startIndex = 0) => {
        if (tracks.length === 0) return;
        set({ queue: tracks, currentIndex: startIndex, isPlaying: true, currentTime: 0, pendingSeek: null });
        useUIStore.getState().autoOpenPanelOnce();
      },

      togglePlay: () => set((s) => ({ isPlaying: !s.isPlaying })),

      next: () => {
        const { queue, currentIndex, shuffle, repeat } = get();
        if (queue.length === 0) return;
        if (repeat === 'one') {
          // restart current track (the bridge consumes pendingSeek)
          set({ currentTime: 0, isPlaying: true, pendingSeek: 0 });
          return;
        }
        if (shuffle) {
          if (queue.length === 1) {
            set({ currentTime: 0, isPlaying: true, pendingSeek: 0 });
            return;
          }
          let nextIdx = currentIndex;
          while (nextIdx === currentIndex) {
            nextIdx = Math.floor(Math.random() * queue.length);
          }
          set({ currentIndex: nextIdx, currentTime: 0, isPlaying: true, pendingSeek: null });
          return;
        }
        let nextIdx = currentIndex + 1;
        if (nextIdx >= queue.length) {
          if (repeat === 'all') {
            nextIdx = 0;
          } else {
            set({ isPlaying: false });
            return;
          }
        }
        set({ currentIndex: nextIdx, currentTime: 0, isPlaying: true, pendingSeek: null });
      },

      prev: () => {
        const { queue, currentIndex, currentTime } = get();
        if (queue.length === 0) return;
        // if past 3s, restart current track (applied to the audio via pendingSeek)
        if (currentTime > 3) {
          set({ currentTime: 0, pendingSeek: 0 });
          return;
        }
        let prevIdx = currentIndex - 1;
        if (prevIdx < 0) prevIdx = 0;
        set({ currentIndex: prevIdx, currentTime: 0, isPlaying: true, pendingSeek: null });
      },

      seek: (time) => set({ currentTime: time, pendingSeek: time }),
      setCurrentTime: (time) => set({ currentTime: time }),
      setDuration: (duration) => set({ duration }),
      setVolume: (v) => set({ volume: Math.max(0, Math.min(1, v)), muted: false }),
      toggleMute: () => set((s) => ({ muted: !s.muted })),
      toggleShuffle: () =>
        set((s) => {
          if (!s.shuffle) {
            const current = s.queue[s.currentIndex];
            const shuffled = current
              ? [current, ...shuffleArray(s.queue.filter((_, i) => i !== s.currentIndex))]
              : shuffleArray(s.queue);
            return { shuffle: true, queue: shuffled, currentIndex: 0 };
          }
          return { shuffle: false };
        }),
      cycleRepeat: () =>
        set((s) => ({
          repeat: s.repeat === 'off' ? 'all' : s.repeat === 'all' ? 'one' : 'off',
        })),

      addToQueue: (track) =>
        set((s) => ({ queue: [...s.queue, track] })),

      removeFromQueue: (index) =>
        set((s) => {
          if (index < 0 || index >= s.queue.length) return {};
          const queue = s.queue.filter((_, i) => i !== index);
          let currentIndex = s.currentIndex;
          if (index < currentIndex) currentIndex--;
          if (queue.length === 0) {
            return { queue: [], currentIndex: 0, isPlaying: false, pendingSeek: null };
          }
          if (currentIndex >= queue.length) currentIndex = queue.length - 1;
          return { queue, currentIndex };
        }),
    }),
    {
      name: 'localwave-player',
      storage: createJSONStorage(() => localStorage),
      // persist only user prefs, not playback state
      partialize: (s) => ({ volume: s.volume, muted: s.muted, shuffle: s.shuffle, repeat: s.repeat }),
    },
  ),
);
