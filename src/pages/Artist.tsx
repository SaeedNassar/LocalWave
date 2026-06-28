import { useEffect, useState } from 'react';
import { useParams } from 'react-router-dom';
import { api } from '../lib/api';
import type { ArtistDetail, ArtistImageResult } from '../types';
import { AlbumGrid } from '../components/AlbumGrid';
import { TrackList } from '../components/TrackList';
import { usePlayerStore } from '../store/player';

export function ArtistPage() {
  const { id } = useParams<{ id: string }>();
  const [detail, setDetail] = useState<ArtistDetail | null>(null);
  const [artistImage, setArtistImage] = useState<ArtistImageResult | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const playQueue = usePlayerStore((s) => s.playQueue);
  const toggleShuffle = usePlayerStore((s) => s.toggleShuffle);

  useEffect(() => {
    const artistId = Number(id);
    setLoading(true);
    setError(null);
    api
      .getArtist(artistId)
      .then((d) => {
        setDetail(d);
        setLoading(false);
        // fetch real artist image (best-effort, non-blocking)
        api
          .getArtistImage(artistId)
          .then(setArtistImage)
          .catch(() => {});
      })
      .catch((err) => {
        setError((err as Error).message);
        setLoading(false);
      });
  }, [id]);

  if (loading) return <div className="p-6 text-sm text-ink-faint">Loading artist…</div>;
  if (error) return <div className="p-6 text-sm text-semantic-neg">{error}</div>;
  if (!detail) return null;

  const { artist, albums, appearsOn, primaryTracks, featuredTracks } = detail;
  const allTracks = [...primaryTracks, ...featuredTracks];
  const imageUrl = artistImage?.imageUrl ?? null;

  const handlePlayAll = () => {
    playQueue(allTracks, 0);
  };

  const handleShuffle = () => {
    // Play first, THEN enable shuffle — otherwise toggleShuffle reorders the
    // old queue and playQueue immediately overwrites it with the in-order list.
    playQueue(allTracks, 0);
    if (!usePlayerStore.getState().shuffle) toggleShuffle();
  };

  return (
    <div className="space-y-8 p-6">
      {/* header */}
      <header className="flex items-end gap-6">
        <div
          className="flex h-48 w-48 shrink-0 items-center justify-center overflow-hidden rounded-full shadow-dialog"
          style={
            !imageUrl
              ? {
                  background: `linear-gradient(135deg, hsl(${(artist.id * 67) % 360}, 30%, 25%), hsl(${(artist.id * 67 + 60) % 360}, 30%, 15%))`,
                }
              : undefined
          }
        >
          {imageUrl ? (
            <img
              src={imageUrl}
              alt={artist.name}
              className="h-full w-full object-cover"
            />
          ) : (
            <span className="text-6xl font-bold text-white/40">
              {artist.name.charAt(0).toUpperCase()}
            </span>
          )}
        </div>
        <div>
          <div className="text-xs font-bold uppercase tracking-button text-ink-muted">Artist</div>
          <h1 className="mt-1 font-title text-4xl font-bold text-ink-base sm:text-5xl">{artist.name}</h1>
          <p className="mt-3 text-sm text-ink-muted">
            {albums.length} album{albums.length !== 1 ? 's' : ''} •{' '}
            {detail.totalTrackCount} track{detail.totalTrackCount !== 1 ? 's' : ''}
          </p>
        </div>
      </header>

      {/* play controls */}
      <div className="flex gap-3">
        <button onClick={handlePlayAll} className="btn-brand" disabled={allTracks.length === 0}>
          ▶ Play
        </button>
        <button onClick={handleShuffle} className="btn-pill" disabled={allTracks.length === 0}>
          ⤮ Shuffle
        </button>
      </div>

      {/* albums (primary) */}
      {albums.length > 0 && (
        <section>
          <h2 className="mb-3 font-title text-lg font-bold text-ink-base">Albums</h2>
          <AlbumGrid albums={albums} />
        </section>
      )}

      {/* appears on */}
      {appearsOn.length > 0 && (
        <section>
          <h2 className="mb-3 font-title text-lg font-bold text-ink-base">Appears On</h2>
          <AlbumGrid albums={appearsOn} />
        </section>
      )}

      {/* popular tracks — primary first */}
      {primaryTracks.length > 0 && (
        <section>
          <h2 className="mb-3 font-title text-lg font-bold text-ink-base">Songs</h2>
          <TrackList tracks={primaryTracks.slice(0, 10)} />
        </section>
      )}

      {/* featured tracks */}
      {featuredTracks.length > 0 && (
        <section>
          <h2 className="mb-3 font-title text-lg font-bold text-ink-base">Featured On</h2>
          <TrackList tracks={featuredTracks} />
        </section>
      )}

      {/* full list */}
      {allTracks.length > 10 && (
        <section>
          <h2 className="mb-3 font-title text-lg font-bold text-ink-base">All Tracks</h2>
          <TrackList tracks={allTracks} />
        </section>
      )}

      {allTracks.length === 0 && (
        <div className="py-12 text-center text-sm text-ink-faint">No tracks found.</div>
      )}
    </div>
  );
}
