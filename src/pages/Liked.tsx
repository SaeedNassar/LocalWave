import { useEffect, useState } from 'react';
import { api } from '../lib/api';
import type { Track } from '../types';
import { TrackList } from '../components/TrackList';
import { HeartFilledIcon } from '../components/icons';
import { usePlayerStore } from '../store/player';

export function LikedPage() {
  const [tracks, setTracks] = useState<Track[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const playQueue = usePlayerStore((s) => s.playQueue);

  useEffect(() => {
    let cancelled = false;
    api
      .getLiked()
      .then((r) => {
        if (cancelled) return;
        setTracks(r.items);
      })
      .catch((err: Error) => {
        if (!cancelled) setError(err.message);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <div className="space-y-6 p-6">
      <header className="flex items-end gap-6">
        <div className="flex h-40 w-40 items-center justify-center rounded-md bg-gradient-to-br from-purple-700 to-brand-dark shadow-elev">
          <HeartFilledIcon width={64} height={64} className="text-white" />
        </div>
        <div>
          <div className="text-xs font-bold uppercase tracking-button text-ink-muted">Auto playlist</div>
          <h1 className="mt-1 font-title text-4xl font-bold text-ink-base">Liked Songs</h1>
          <p className="mt-2 text-sm text-ink-muted">{tracks.length} tracks you've liked</p>
        </div>
      </header>

      {tracks.length > 0 && (
        <div className="flex gap-3">
          <button onClick={() => playQueue(tracks, 0)} className="btn-brand">
            ▶ Play
          </button>
        </div>
      )}

      {loading ? (
        error ? (
          <div className="py-12 text-center text-sm text-semantic-warn">{error}</div>
        ) : (
          <div className="py-12 text-center text-sm text-ink-faint">Loading…</div>
        )
      ) : (
        <TrackList tracks={tracks} emptyMessage="No liked songs yet. Click the ♥ on a track." />
      )}
    </div>
  );
}
