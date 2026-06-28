import { useEffect, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { api } from '../lib/api';
import { useLibraryStore } from '../store/library';
import { usePlayerStore } from '../store/player';
import type { Track } from '../types';

interface TrackContextMenuProps {
  track: Track;
  x: number;
  y: number;
  onClose: () => void;
  onEditMetadata?: () => void;
}

export function TrackContextMenu({ track, x, y, onClose, onEditMetadata }: TrackContextMenuProps) {
  const navigate = useNavigate();
  const playlists = useLibraryStore((s) => s.playlists);
  const loadPlaylists = useLibraryStore((s) => s.loadPlaylists);
  const toggleLikeOptimistic = useLibraryStore((s) => s.toggleLikeOptimistic);
  const addToQueue = usePlayerStore((s) => s.addToQueue);
  const ref = useRef<HTMLDivElement>(null);
  const [activeSubmenu, setActiveSubmenu] = useState<'playlists' | 'artists' | null>(null);
  const [submenuTop, setSubmenuTop] = useState(0);
  const [creating, setCreating] = useState(false);
  const [newName, setNewName] = useState('');

  useEffect(() => {
    function onClick(e: MouseEvent) {
      if (!ref.current?.contains(e.target as Node)) {
        onClose();
      }
    }
    function onKey(e: KeyboardEvent) {
      if (e.key === 'Escape') onClose();
    }
    const clickTimer = setTimeout(() => {
      window.addEventListener('click', onClick);
      window.addEventListener('keydown', onKey);
    }, 50);
    return () => {
      clearTimeout(clickTimer);
      window.removeEventListener('click', onClick);
      window.removeEventListener('keydown', onKey);
    };
  }, [onClose]);

  const style: React.CSSProperties = {
    position: 'fixed',
    left: Math.min(x, window.innerWidth - 260),
    top: Math.min(y, window.innerHeight - 320),
    zIndex: 50,
  };

  const handleAddToPlaylist = async (playlistId: number) => {
    try {
      await api.addTracksToPlaylist(playlistId, [track.id]);
      await loadPlaylists();
    } catch (err) {
      alert((err as Error).message);
    }
    onClose();
  };

  const handleCreatePlaylist = async () => {
    if (!newName.trim()) {
      setCreating(false);
      return;
    }
    try {
      const p = await api.createPlaylist(newName.trim());
      await api.addTracksToPlaylist(p.id, [track.id]);
      await loadPlaylists();
    } catch (err) {
      alert((err as Error).message);
    }
    onClose();
  };

  const handleToggleLike = async () => {
    const next = !track.liked;
    toggleLikeOptimistic(track.id);
    try {
      await api.toggleLike(track.id, next);
    } catch {
      toggleLikeOptimistic(track.id);
    }
    onClose();
  };

  const handleAddToQueue = () => {
    addToQueue(track);
    onClose();
  };

  const handleGoToAlbum = () => {
    if (track.albumId) navigate(`/album/${track.albumId}`);
    onClose();
  };

  const handleGoToArtist = (artistId: number) => {
    navigate(`/artist/${artistId}`);
    onClose();
  };

  const artists =
    track.artists.length > 0
      ? track.artists
      : track.artistId
        ? [{ id: track.artistId, name: track.artist ?? 'Unknown Artist' }]
        : [];

  const openSubmenu = (submenu: 'playlists' | 'artists', el: HTMLButtonElement) => {
    setSubmenuTop(el.offsetTop);
    setActiveSubmenu(submenu);
  };

  return (
    <>
    <div
      ref={ref}
      className="rounded-md bg-surface shadow-dialog ring-1 ring-white/10 py-1 w-56 text-sm"
      style={style}
      onMouseLeave={() => setActiveSubmenu(null)}
      onClick={(e) => e.stopPropagation()}
    >
      <button
        className={`w-full px-4 py-2 text-left hover:bg-surface-mid ${activeSubmenu === 'playlists' ? 'bg-surface-mid text-brand' : 'text-ink-base'}`}
        onMouseEnter={(e) => openSubmenu('playlists', e.currentTarget)}
      >
        Add to playlist ▶
      </button>

      {track.liked ? (
        <button
          className="w-full px-4 py-2 text-left text-ink-base hover:bg-surface-mid"
          onClick={handleToggleLike}
          onMouseEnter={() => setActiveSubmenu(null)}
        >
          Remove from liked songs
        </button>
      ) : (
        <button
          className="w-full px-4 py-2 text-left text-ink-base hover:bg-surface-mid"
          onClick={handleToggleLike}
          onMouseEnter={() => setActiveSubmenu(null)}
        >
          Add to liked songs
        </button>
      )}

      <button
        className="w-full px-4 py-2 text-left text-ink-base hover:bg-surface-mid"
        onClick={handleAddToQueue}
        onMouseEnter={() => setActiveSubmenu(null)}
      >
        Add to queue
      </button>

      {artists.length === 1 ? (
        <button
          className="w-full px-4 py-2 text-left text-ink-base hover:bg-surface-mid"
          onClick={() => handleGoToArtist(artists[0].id)}
          onMouseEnter={() => setActiveSubmenu(null)}
        >
          Go to artist
        </button>
      ) : artists.length > 1 ? (
        <button
          className={`w-full px-4 py-2 text-left hover:bg-surface-mid ${activeSubmenu === 'artists' ? 'bg-surface-mid text-brand' : 'text-ink-base'}`}
          onMouseEnter={(e) => openSubmenu('artists', e.currentTarget)}
        >
          Go to artist ▶
        </button>
      ) : null}

      {track.albumId && (
        <button
          className="w-full px-4 py-2 text-left text-ink-base hover:bg-surface-mid"
          onClick={handleGoToAlbum}
          onMouseEnter={() => setActiveSubmenu(null)}
        >
          Go to album
        </button>
      )}

      <button
        className="w-full px-4 py-2 text-left text-ink-base hover:bg-surface-mid"
        onClick={() => {
          onClose();
          onEditMetadata?.();
        }}
        onMouseEnter={() => setActiveSubmenu(null)}
      >
        Edit metadata
      </button>

      {activeSubmenu === 'playlists' && (
        <div
          className="absolute left-full ml-1 rounded-md bg-surface shadow-dialog ring-1 ring-white/10 py-1 w-56 max-h-72 overflow-y-auto"
          style={{ top: submenuTop }}
        >
          {playlists.length === 0 && !creating && (
            <div className="px-4 py-2 text-xs text-ink-muted">No playlists yet.</div>
          )}
          {playlists.map((p) => (
            <button
              key={p.id}
              className="w-full px-4 py-2 text-left text-ink-base hover:bg-surface-mid truncate"
              onClick={() => handleAddToPlaylist(p.id)}
              title={p.name}
            >
              {p.name}
            </button>
          ))}
          {creating ? (
            <div className="px-3 py-2">
              <input
                autoFocus
                value={newName}
                onChange={(e) => setNewName(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') handleCreatePlaylist();
                  if (e.key === 'Escape') setCreating(false);
                }}
                onBlur={handleCreatePlaylist}
                placeholder="New playlist…"
                className="w-full rounded bg-surface-mid px-2 py-1 text-xs text-ink-base placeholder:text-ink-faint focus:outline-none focus:ring-1 focus:ring-brand"
              />
            </div>
          ) : (
            <button
              className="w-full px-4 py-2 text-left text-brand hover:bg-surface-mid"
              onClick={() => setCreating(true)}
            >
              + Create new playlist
            </button>
          )}
        </div>
      )}

      {activeSubmenu === 'artists' && artists.length > 1 && (
        <div
          className="absolute left-full ml-1 rounded-md bg-surface shadow-dialog ring-1 ring-white/10 py-1 w-48 max-h-72 overflow-y-auto"
          style={{ top: submenuTop }}
        >
          {artists.map((a) => (
            <button
              key={a.id}
              className="w-full px-4 py-2 text-left text-ink-base hover:bg-surface-mid truncate"
              onClick={() => handleGoToArtist(a.id)}
              title={a.name}
            >
              {a.name}
            </button>
          ))}
        </div>
      )}
    </div>

  </>
  );
}
