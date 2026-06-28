import { Link } from 'react-router-dom';
import { useLibraryStore } from '../store/library';
import { usePlayerStore } from '../store/player';
import { AlbumGrid } from '../components/AlbumGrid';
import { TrackList } from '../components/TrackList';
import { ImportPlaylistButton } from '../components/SearchBar';
import { API_ORIGIN } from '../lib/api';

export function HomePage() {
  const { tracks, albums, artists, scanStatus, loadPlaylists } = useLibraryStore();
  const playQueue = usePlayerStore((s) => s.playQueue);
  const toggleShuffle = usePlayerStore((s) => s.toggleShuffle);

  const recentTracks = [...tracks]
    .sort((a, b) => new Date(b.dateAdded).getTime() - new Date(a.dateAdded).getTime())
    .slice(0, 10);

  const greeting = (() => {
    const h = new Date().getHours();
    if (h < 12) return 'Good morning';
    if (h < 18) return 'Good afternoon';
    return 'Good evening';
  })();

  return (
    <div className="space-y-8 p-6">
      <header className="flex flex-wrap items-center justify-between gap-4">
        <div>
          <h1 className="font-title text-2xl font-bold text-ink-base">{greeting}</h1>
          {scanStatus && (
            <p className="mt-1 text-xs text-ink-muted">
              {scanStatus.tracks} tracks • {scanStatus.albums} albums • {artists.length} artists
            </p>
          )}
        </div>
        <div className="flex items-center gap-3">
          <Link to="/search" className="btn-pill text-ink-muted">
            Search…
          </Link>
          <Link to="/settings" className="btn-pill">
            Settings
          </Link>
          <ImportPlaylistButton onImported={loadPlaylists} />
        </div>
      </header>

      {/* quick access tiles */}
      <section>
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {recentTracks.slice(0, 6).map((t) => (
            <button
              key={t.id}
              onClick={() => playQueue([t])}
              className="group flex items-center gap-4 overflow-hidden rounded-md bg-surface pr-4 text-left transition-colors hover:bg-surface-card"
            >
              <div className="h-16 w-16 shrink-0">
                <CoverTile id={t.id} />
              </div>
              <span className="truncate text-sm font-bold text-ink-base">{t.title}</span>
              <span className="ml-auto hidden rounded-full bg-brand p-3 text-base opacity-0 transition-opacity group-hover:opacity-100 lg:block">
                ▶
              </span>
            </button>
          ))}
        </div>
      </section>

      <section>
        <div className="mb-3 flex items-center justify-between">
          <h2 className="font-title text-lg font-bold text-ink-base">Recently added</h2>
          <button
            onClick={() => recentTracks.length && playQueue(recentTracks, 0)}
            className="btn-pill text-xs"
          >
            Play all
          </button>
        </div>
        <TrackList tracks={recentTracks} showHeader />
      </section>

      <section>
        <h2 className="mb-3 font-title text-lg font-bold text-ink-base">Albums</h2>
        <AlbumGrid albums={albums.slice(0, 10)} emptyMessage="No albums yet — scan your library." />
      </section>
    </div>
  );
}

function CoverTile({ id }: { id: number }) {
  return (
    <div className="h-16 w-16">
      <img
        src={`${API_ORIGIN}/api/cover/${id}`}
        alt=""
        className="h-full w-full object-cover"
        loading="lazy"
        onError={(e) => {
          (e.currentTarget as HTMLImageElement).style.display = 'none';
        }}
      />
    </div>
  );
}
