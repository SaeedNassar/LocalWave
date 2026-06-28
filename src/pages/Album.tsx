import { useEffect, useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import { api, API_ORIGIN } from '../lib/api';
import type { Album, Track } from '../types';
import { TrackList } from '../components/TrackList';
import { ArtistLinks } from '../components/ArtistLinks';
import { usePlayerStore } from '../store/player';

export function AlbumPage() {
  const { id } = useParams<{ id: string }>();
  const [album, setAlbum] = useState<Album | null>(null);
  const [tracks, setTracks] = useState<Track[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const playQueue = usePlayerStore((s) => s.playQueue);

  useEffect(() => {
    const albumId = Number(id);
    if (!Number.isFinite(albumId)) {
      setError('Invalid album id');
      setLoading(false);
      return;
    }
    let cancelled = false;
    setLoading(true);
    api
      .getAlbum(albumId)
      .then((r) => {
        if (cancelled) return;
        setAlbum(r.album);
        setTracks(r.tracks);
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
  }, [id]);

  if (loading || !album) {
    if (error) return <div className="p-6 text-sm text-semantic-warn">{error}</div>;
    return <div className="p-6 text-sm text-ink-faint">Loading album…</div>;
  }

  return (
    <div className="space-y-6 p-6">
      <header className="flex items-end gap-6">
        <div className="h-40 w-40 overflow-hidden rounded-md shadow-elev">
          {album.coverTrackId && (
            <img
              src={`${API_ORIGIN}/api/cover/${album.coverTrackId}`}
              alt=""
              className="h-full w-full object-cover"
              onError={(e) => {
                (e.currentTarget as HTMLImageElement).style.display = 'none';
              }}
            />
          )}
        </div>
        <div>
          <div className="text-xs font-bold uppercase tracking-button text-ink-muted">Album</div>
          <h1 className="mt-1 font-title text-4xl font-bold text-ink-base">{album.name}</h1>
          <p className="mt-2 text-sm text-ink-muted">
            <ArtistLinks raw={album.albumArtist} artistId={album.artistId} linkClassName="hover:text-ink-base" />
            {album.year ? ` • ${album.year}` : ''}
            {` • ${tracks.length} tracks`}
          </p>
        </div>
      </header>

      <div className="flex gap-3">
        <button onClick={() => playQueue(tracks, 0)} className="btn-brand" disabled={tracks.length === 0}>
          ▶ Play
        </button>
      </div>

      <TrackList tracks={tracks} showAlbum={false} />
    </div>
  );
}
