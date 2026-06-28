import { NavLink, useNavigate } from 'react-router-dom';
import { useState } from 'react';
import { useLibraryStore } from '../store/library';
import { api } from '../lib/api';
import {
  HomeIcon,
  HeartFilledIcon,
  LibraryIcon,
  PlusIcon,
  SearchIcon,
} from './icons';
import type { Playlist } from '../types';

export function Sidebar() {
  const playlists = useLibraryStore((s) => s.playlists);
  const loadPlaylists = useLibraryStore((s) => s.loadPlaylists);
  const navigate = useNavigate();
  const [creating, setCreating] = useState(false);
  const [name, setName] = useState('');

  const handleCreate = async () => {
    if (!name.trim()) {
      setCreating(false);
      return;
    }
    try {
      const p = await api.createPlaylist(name.trim());
      await loadPlaylists();
      setName('');
      setCreating(false);
      navigate(`/playlist/${p.id}`);
    } catch (err) {
      alert((err as Error).message);
    }
  };

  return (
    <aside className="flex h-full w-64 shrink-0 flex-col gap-2 bg-base p-2">
      {/* top nav block */}
      <nav className="rounded-lg bg-surface p-2">
        <SidebarLink to="/" icon={<HomeIcon width={24} height={24} />} label="Home" end />
        <SidebarLink to="/search" icon={<SearchIcon width={24} height={24} />} label="Search" />
      </nav>

      {/* library block */}
      <div className="flex min-h-0 flex-1 flex-col rounded-lg bg-surface">
        <div className="flex items-center justify-between px-4 pt-4 pb-2">
          <NavLink
            to="/library"
            className="nav-link nav-link-active gap-3 font-bold text-ink-muted transition-colors hover:text-ink-base"
          >
            <LibraryIcon width={24} height={24} />
            <span>Your Library</span>
          </NavLink>
          <button
            onClick={() => setCreating(true)}
            className="rounded-pill bg-surface-mid p-2 text-ink-base hover:bg-surface-card"
            aria-label="Create playlist"
            title="Create playlist"
          >
            <PlusIcon />
          </button>
        </div>

        {/* liked songs pseudo-playlist */}
        <div className="px-2">
          <PlaylistRow
            to="/liked"
            icon={
              <div className="flex h-12 w-12 items-center justify-center rounded-md bg-gradient-to-br from-brand-dark to-purple-700 text-white">
                <HeartFilledIcon width={20} height={20} />
              </div>
            }
            title="Liked Songs"
            subtitle="Auto playlist"
          />
        </div>

        {creating && (
          <div className="px-3 py-2">
            <input
              autoFocus
              value={name}
              onChange={(e) => setName(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') handleCreate();
                if (e.key === 'Escape') {
                  setCreating(false);
                  setName('');
                }
              }}
              onBlur={() => { if (!name.trim()) setCreating(false); }}
              placeholder="Playlist name…"
              className="w-full rounded-md bg-surface-mid px-3 py-2 text-sm text-ink-base placeholder:text-ink-faint focus:outline-none focus:ring-1 focus:ring-brand"
            />
          </div>
        )}

        <div className="min-h-0 flex-1 overflow-y-auto px-2 pb-2">
          {playlists.length === 0 && !creating && (
            <div className="px-3 py-6 text-center text-xs text-ink-faint">
              No playlists yet. Create one or import an .m3u8.
            </div>
          )}
          {playlists.map((p, i) => (
            <PlaylistRow
              key={p.id}
              to={`/playlist/${p.id}`}
              icon={<PlaylistTile playlist={p} />}
              title={p.name}
              subtitle={`${p.trackCount} tracks${p.isImported ? ' • Imported' : ''}`}
              playlistId={p.id}
              index={i}
              onReorder={async (from, to) => {
                await api.reorderPlaylists(from, to);
              }}
            />
          ))}
        </div>
      </div>
    </aside>
  );
}

function SidebarLink({
  to,
  icon,
  label,
  end,
}: {
  to: string;
  icon: React.ReactNode;
  label: string;
  end?: boolean;
}) {
  return (
    <NavLink
      to={to}
      end={end}
      className={({ isActive }) =>
        `nav-link gap-4 py-3 text-base font-bold ${isActive ? 'text-ink-base' : 'text-ink-muted hover:text-ink-base'}`
      }
    >
      {icon}
      <span>{label}</span>
    </NavLink>
  );
}

function PlaylistRow({
  to,
  icon,
  title,
  subtitle,
  playlistId,
  index,
  onReorder,
}: {
  to: string;
  icon: React.ReactNode;
  title: string;
  subtitle: string;
  playlistId?: number;
  index?: number;
  onReorder?: (from: number, to: number) => void;
}) {
  const loadPlaylists = useLibraryStore((s) => s.loadPlaylists);
  const [dragOver, setDragOver] = useState(false);
  const navigate = useNavigate();

  const isPlaylistReorder = typeof index === 'number' && !!onReorder;

  return (
    <NavLink
      to={to}
      className={({ isActive }) =>
        `flex items-center gap-3 rounded-md p-2 transition-colors hover:bg-surface-mid ${
          isActive ? 'bg-surface-mid' : ''
        } ${dragOver ? 'bg-surface-mid ring-1 ring-brand' : ''}`
      }
      draggable={isPlaylistReorder}
      onDragStart={(e) => {
        if (!isPlaylistReorder) return;
        e.dataTransfer.setData('application/localwave-playlist', JSON.stringify({ index }));
        e.dataTransfer.effectAllowed = 'move';
      }}
      onDragOver={(e) => {
        if (isPlaylistReorder && e.dataTransfer.types.includes('application/localwave-playlist')) {
          e.preventDefault();
          setDragOver(true);
          e.dataTransfer.dropEffect = 'move';
          return;
        }
        if (playlistId == null) return;
        if (e.dataTransfer.types.includes('application/localwave-track')) {
          e.preventDefault();
          setDragOver(true);
          e.dataTransfer.dropEffect = 'copy';
        }
      }}
      onDragLeave={() => setDragOver(false)}
      onDrop={async (e) => {
        e.preventDefault();
        setDragOver(false);

        const playlistData = e.dataTransfer.getData('application/localwave-playlist');
        if (isPlaylistReorder && playlistData) {
          try {
            const { index: fromIndex } = JSON.parse(playlistData) as { index: number };
            if (fromIndex === index) return;
            await onReorder!(fromIndex, index ?? fromIndex);
            await loadPlaylists();
          } catch (err) {
            alert((err as Error).message);
          }
          return;
        }

        if (playlistId == null) return;
        const trackData = e.dataTransfer.getData('application/localwave-track');
        if (!trackData) return;
        try {
          const { trackId } = JSON.parse(trackData) as { trackId: number };
          await api.addTracksToPlaylist(playlistId, [trackId]);
          await loadPlaylists();
          if (to.startsWith('/playlist/')) {
            navigate(to);
          }
        } catch (err) {
          alert((err as Error).message);
        }
      }}
    >
      <div className="h-12 w-12 shrink-0 overflow-hidden rounded-md">{icon}</div>
      <div className="min-w-0">
        <div className="truncate text-sm font-bold text-ink-base">{title}</div>
        <div className="truncate text-xs text-ink-muted">{subtitle}</div>
      </div>
    </NavLink>
  );
}

function PlaylistTile({ playlist }: { playlist: Playlist }) {
  // simple generated gradient from playlist id, no cover stored
  const hue = (playlist.id * 47) % 360;
  return (
    <div
      className="flex h-12 w-12 items-center justify-center rounded-md"
      style={{ background: `hsl(${hue}, 35%, 25%)` }}
    >
      <MusicGlyph />
    </div>
  );
}

function MusicGlyph() {
  return (
    <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor" className="text-white/80">
      <path d="M12 3v10.55A4 4 0 1014 17V7h4V3h-6z" />
    </svg>
  );
}
