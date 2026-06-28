import { create } from 'zustand';
import { persist, createJSONStorage } from 'zustand/middleware';

interface UIState {
  queueOpen: boolean;
  nowPlayingPanelOpen: boolean;
  sidebarCollapsed: boolean;
  hasAutoOpenedPanelOnce: boolean;
  toggleQueue: () => void;
  setQueueOpen: (open: boolean) => void;
  toggleNowPlayingPanel: () => void;
  setNowPlayingPanelOpen: (open: boolean) => void;
  autoOpenPanelOnce: () => void;
  toggleSidebar: () => void;
}

export const useUIStore = create<UIState>()(
  persist(
    (set, get) => ({
      queueOpen: false,
      nowPlayingPanelOpen: false,
      sidebarCollapsed: false,
      hasAutoOpenedPanelOnce: false,
      toggleQueue: () => set((s) => ({ queueOpen: !s.queueOpen })),
      setQueueOpen: (open) => set({ queueOpen: open }),
      toggleNowPlayingPanel: () =>
        set((s) => ({ nowPlayingPanelOpen: !s.nowPlayingPanelOpen })),
      setNowPlayingPanelOpen: (open) =>
        set({ nowPlayingPanelOpen: open }),
      autoOpenPanelOnce: () => {
        if (!get().hasAutoOpenedPanelOnce) {
          set({ nowPlayingPanelOpen: true, hasAutoOpenedPanelOnce: true });
        }
      },
      toggleSidebar: () => set((s) => ({ sidebarCollapsed: !s.sidebarCollapsed })),
    }),
    {
      name: 'localwave-ui',
      storage: createJSONStorage(() => localStorage),
      partialize: (s) => ({ hasAutoOpenedPanelOnce: s.hasAutoOpenedPanelOnce }),
    },
  ),
);
