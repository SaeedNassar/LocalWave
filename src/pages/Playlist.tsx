import { useEffect, useState } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { api } from '../lib/api';
import type { PlaylistDetail, PlaylistEntry, Track } from '../types';
import { TrackRow } from '../components/TrackRow';
import { ImportPlaylistButton } from '../components/SearchBar';
import { useLibraryStore } from '../store/library';
import { usePlayerStore } from '../store/player';

export function PlaylistPage() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const [detail, setDetail] = useState<PlaylistDetail | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [editing, setEditing] = useState(false);
  const [name, setName] = useState('');
  const loadPlaylists = useLibraryStore((s) => s.loadPlaylists);
  const playQueue = usePlayerStore((s) => s.playQueue);

  const playlistId = Number(id);

  const load = () => {
    if (!Number.isFinite(playlistId)) {
      setLoading(false);
      return;
    }
    let cancelled = false;
    setLoading(true);
    api
      .getPlaylist(playlistId)
      .then((d) => {
        if (cancelled) return;
        setDetail(d);
        setName(d.name);
      })
      .catch((err: Error) => setError(err.message))
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  };

  useEffect(() => load(), [playlistId]);

  const handleSave = async () => {
    if (!detail || !name.trim()) return;
    await api.renamePlaylist(detail.id, name.trim());
    await loadPlaylists();
    setEditing(false);
    load();
  };

  const handleDelete = async () => {
    if (!detail) return;
    if (!window.confirm(`Delete playlist "${detail.name}"? This cannot be undone.`)) return;
    await api.deletePlaylist(detail.id);
    await loadPlaylists();
    navigate('/');
  };

  if (loading || !detail) {
    if (error) return <div className="p-6 text-sm text-semantic-warn">{error}</div>;
    return <div className="p-6 text-sm text-ink-faint">Loading playlist…</div>;
  }

  const tracks = detail.entries
    .filter((e) => e.track !== null)
    .map((e) => e.track!) as import('../types').Track[];
  const missingCount = detail.entries.filter((e) => e.missing).length;

  return (
    <div className="space-y-6 p-6">
      <header className="flex items-end gap-6">
        <div
          className="flex h-40 w-40 items-center justify-center rounded-md shadow-elev"
          style={{
            background: detail.isImported
              ? 'linear-gradient(135deg, #4a3a8a, #1a1a40)'
              : `hsl(${(detail.id * 47) % 360}, 35%, 25%)`,
          }}
        >
          <span className="text-4xl">🎵</span>
        </div>
        <div className="flex-1">
          <div className="text-xs font-bold uppercase tracking-button text-ink-muted">
            {detail.isImported ? 'Imported playlist' : 'Playlist'}
          </div>
          {editing ? (
            <input
              autoFocus
              value={name}
              onChange={(e) => setName(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') handleSave();
                if (e.key === 'Escape') setEditing(false);
              }}
              onBlur={handleSave}
              className="mt-1 w-full max-w-md rounded-md bg-surface-mid px-3 py-2 font-title text-2xl font-bold text-ink-base focus:outline-none focus:ring-1 focus:ring-brand"
            />
          ) : (
            <h1
              onClick={() => setEditing(true)}
              className="mt-1 cursor-text font-title text-4xl font-bold text-ink-base"
              title="Click to rename"
            >
              {detail.name}
            </h1>
          )}
          <p className="mt-2 text-sm text-ink-muted">
            {detail.trackCount} tracks
            {missingCount > 0 && (
              <span className="text-semantic-warn"> • {missingCount} missing</span>
            )}
          </p>
        </div>
      </header>

      <div className="flex flex-wrap gap-3">
        <button onClick={() => playQueue(tracks, 0)} className="btn-brand" disabled={tracks.length === 0}>
          ▶ Play
        </button>
        <button onClick={() => setEditing(true)} className="btn-pill">
          Rename
        </button>
        <button onClick={handleDelete} className="btn-outlined">
          Delete
        </button>
        <ImportPlaylistButton onImported={loadPlaylists} />
      </div>

      {missingCount > 0 && (
        <div className="rounded-md bg-semantic-warn/10 border border-semantic-warn/30 px-4 py-3 text-sm text-semantic-warn">
          {missingCount} entr{missingCount === 1 ? 'y' : 'ies'} from the .m3u8 could not be matched to a
          file in your library. They're listed below as "missing".
        </div>
      )}

      <ReorderablePlaylistEntries
        detail={detail}
        playlistId={playlistId}
        onUpdate={setDetail}
      />

      {missingCount > 0 && (
        <section>
          <h2 className="mb-3 font-title text-base font-bold text-ink-muted">Missing entries</h2>
          <ul className="space-y-2">
            {detail.entries
              .filter((e) => e.missing)
              .map((e, i) => (
                <li
                  key={i}
                  className="rounded-md bg-surface px-3 py-2 text-xs text-ink-muted"
                >
                  <span className="font-bold text-semantic-warn">✕ </span>
                  {e.title ?? e.rawEntry}
                  <span className="block text-ink-faint">{e.rawEntry}</span>
                </li>
              ))}
          </ul>
        </section>
      )}
    </div>
  );
}

function ReorderablePlaylistEntries({
  detail,
  playlistId,
  onUpdate,
}: {
  detail: PlaylistDetail;
  playlistId: number;
  onUpdate: (d: PlaylistDetail) => void;
}) {
  const [dragOverIndex, setDragOverIndex] = useState<number | null>(null);
  const [draggingIndex, setDraggingIndex] = useState<number | null>(null);

  // Only the visible (non-missing) entries are reorderable.
  const visibleEntries = detail.entries.filter((e): e is PlaylistEntry & { track: Track } => e.track !== null);

  if (visibleEntries.length === 0) {
    return <div className="py-12 text-center text-sm text-ink-faint">This playlist is empty.</div>;
  }

  const handleDragStart = (index: number) => {
    setDraggingIndex(index);
  };

  const handleDrop = async (toVisibleIndex: number) => {
    if (draggingIndex == null || draggingIndex === toVisibleIndex) {
      setDraggingIndex(null);
      setDragOverIndex(null);
      return;
    }

    // Map visible indices to full-list indices, accounting for missing entries.
    const fromFull = visibleToFullIndex(draggingIndex, detail.entries);
    const toFull = visibleToFullIndex(toVisibleIndex, detail.entries);

    // Optimistically reorder the full entries list for instant feedback.
    const reordered = [...detail.entries];
    const [moved] = reordered.splice(fromFull, 1);
    const insertAt = Math.max(0, Math.min(toFull, reordered.length));
    reordered.splice(insertAt, 0, moved);
    onUpdate({ ...detail, entries: reordered });

    try {
      await api.reorderPlaylist(playlistId, fromFull, toFull);
    } catch (err) {
      alert((err as Error).message);
    } finally {
      setDraggingIndex(null);
      setDragOverIndex(null);
    }
  };

  return (
    <div className="space-y-1">
      {visibleEntries.map((entry, visibleIndex) => (
        <div
          key={`${entry.track.id}-${entry.position}`}
          className={`relative rounded-md ${dragOverIndex === visibleIndex ? 'bg-surface-mid/80' : ''}`}
          onDragOver={(e) => {
            if (e.dataTransfer.types.includes('application/localwave-track-reorder')) {
              e.preventDefault();
              e.dataTransfer.dropEffect = 'move';
              setDragOverIndex(visibleIndex);
            }
          }}
          onDragLeave={() => setDragOverIndex(null)}
          onDrop={(e) => {
            e.preventDefault();
            if (e.dataTransfer.types.includes('application/localwave-track-reorder')) {
              void handleDrop(visibleIndex);
            }
          }}
        >
          {dragOverIndex === visibleIndex && (
            <div className="pointer-events-none absolute -top-0.5 left-0 right-0 h-0.5 bg-brand" />
          )}
          <div
            draggable
            onDragStart={(e) => {
              e.dataTransfer.setData(
                'application/localwave-track-reorder',
                JSON.stringify({ visibleIndex }),
              );
              e.dataTransfer.effectAllowed = 'move';
              handleDragStart(visibleIndex);
            }}
            onDragEnd={() => {
              setDraggingIndex(null);
              setDragOverIndex(null);
            }}
          >
            <TrackRow
              track={entry.track}
              index={visibleIndex}
              queue={visibleEntries.map((e) => e.track)}
              draggable={false}
            />
          </div>
        </div>
      ))}
    </div>
  );
}

function visibleToFullIndex(visibleIndex: number, entries: PlaylistEntry[]): number {
  let seen = 0;
  for (let i = 0; i < entries.length; i++) {
    if (!entries[i].missing) {
      if (seen === visibleIndex) return i;
      seen++;
    }
  }
  return entries.length;
}
