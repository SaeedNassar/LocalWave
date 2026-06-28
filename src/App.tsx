import { Routes, Route } from 'react-router-dom';
import { useEffect } from 'react';
import { Sidebar } from './components/Sidebar';
import { NowPlayingBar } from './components/NowPlayingBar';
import { Queue } from './components/Queue';
import { NowPlayingPanelDrawer } from './components/NowPlayingPanelDrawer';
import { usePlayer } from './hooks/usePlayer';
import { useLibraryStore } from './store/library';
import { HomePage } from './pages/Home';
import { SearchPage } from './pages/Search';
import { LibraryPage } from './pages/Library';
import { LikedPage } from './pages/Liked';
import { PlaylistPage } from './pages/Playlist';
import { AlbumPage } from './pages/Album';
import { ArtistPage } from './pages/Artist';
import { SettingsPage } from './pages/Settings';

export default function App() {
  // mount the singleton audio bridge
  usePlayer();

  // load library data once on app boot
  useEffect(() => {
    useLibraryStore.getState().loadAll();
  }, []);

  // The backend's initial scan (which auto-imports playlists) runs after the
  // server starts — typically finishing a few seconds after the frontend boots.
  // Poll scan status; when playlist count changes, reload the sidebar's list.
  useEffect(() => {
    let prev: number | null = null;
    const timer = setInterval(async () => {
      try {
        await useLibraryStore.getState().refreshStatus();
        const cur = useLibraryStore.getState().scanStatus?.playlists ?? 0;
        if (prev !== null && cur !== prev) {
          await useLibraryStore.getState().loadPlaylists();
        }
        prev = cur;
      } catch {
        /* ignore */
      }
    }, 3000);
    return () => clearInterval(timer);
  }, []);

  return (
    <div className="flex h-screen flex-col bg-base">
      <div className="relative flex min-h-0 flex-1 gap-2 p-2 pb-0">
        <Sidebar />
        <main className="min-w-0 flex-1 overflow-hidden rounded-lg bg-base">
          <div className="h-full overflow-y-auto">
            <Routes>
              <Route path="/" element={<HomePage />} />
              <Route path="/search" element={<SearchPage />} />
              <Route path="/library" element={<LibraryPage />} />
              <Route path="/liked" element={<LikedPage />} />
              <Route path="/playlist/:id" element={<PlaylistPage />} />
              <Route path="/album/:id" element={<AlbumPage />} />
              <Route path="/artist/:id" element={<ArtistPage />} />
              <Route path="/settings" element={<SettingsPage />} />
            </Routes>
          </div>
        </main>
        <Queue />
        <NowPlayingPanelDrawer />
      </div>
      <NowPlayingBar />
    </div>
  );
}
