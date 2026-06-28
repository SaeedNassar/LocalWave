import { useNavigate } from 'react-router-dom';
import { useEffect, useRef, useState } from 'react';
import { useLibraryStore } from '../store/library';
import { api } from '../lib/api';
import type { Artist } from '../types';
import { AlbumGrid } from '../components/AlbumGrid';
import { TrackList } from '../components/TrackList';

export function LibraryPage() {
  const { tracks, albums, artists } = useLibraryStore();
  const navigate = useNavigate();

  const sortedArtists = [...artists].sort((a, b) => {
    const aUnknown = a.name.toLowerCase() === 'unknown artist' ? 1 : 0;
    const bUnknown = b.name.toLowerCase() === 'unknown artist' ? 1 : 0;
    if (aUnknown !== bUnknown) return aUnknown - bUnknown;
    return a.name.localeCompare(b.name);
  });

  const visibleArtists = sortedArtists.slice(0, 24);

  // Consecutive (not concurrent) artist image fetching
  const [artistImages, setArtistImages] = useState<Record<number, string | null>>({});
  const fetchedRef = useRef(false);

  useEffect(() => {
    if (fetchedRef.current) return;
    fetchedRef.current = true;
    let cancelled = false;

    (async () => {
      for (const a of visibleArtists) {
        if (cancelled) return;
        if (a.imagePath) { setArtistImages((prev) => ({ ...prev, [a.id]: a.imagePath })); continue; }
        try {
          const r = await api.getArtistImage(a.id);
          if (!cancelled) setArtistImages((prev) => ({ ...prev, [a.id]: r.imageUrl ?? null }));
        } catch { if (!cancelled) setArtistImages((prev) => ({ ...prev, [a.id]: null })); }
      }
    })();

    return () => { cancelled = true; };
  }, [visibleArtists.map((a) => a.id).join(',')]);

  return (
    <div className="space-y-8 p-6">
      <h1 className="font-title text-2xl font-bold text-ink-base">Your Library</h1>

      <section>
        <h2 className="mb-3 font-title text-lg font-bold text-ink-base">
          Artists ({artists.length})
        </h2>
        <div className="grid grid-cols-2 gap-4 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-6">
          {visibleArtists.map((a) => (
            <ArtistCard
              key={a.id}
              artist={a}
              img={artistImages[a.id]}
              onClick={() => navigate(`/artist/${a.id}`)}
            />
          ))}
        </div>
      </section>

      <section>
        <h2 className="mb-3 font-title text-lg font-bold text-ink-base">Albums ({albums.length})</h2>
        <AlbumGrid albums={albums} />
      </section>

      <section>
        <h2 className="mb-3 font-title text-lg font-bold text-ink-base">All tracks ({tracks.length})</h2>
        <TrackList tracks={tracks.slice(0, 200)} showHeader />
      </section>
    </div>
  );
}

function ArtistCard({ artist, img, onClick }: { artist: Artist; img: string | null | undefined; onClick: () => void }) {
  const hue = (artist.id * 67) % 360;
  const [imgError, setImgError] = useState(false);
  const showImg = img && !imgError;

  return (
    <button onClick={onClick} className="group card flex flex-col items-center text-center">
      {showImg ? (
        <img
          src={img!}
          alt=""
          className="h-24 w-24 rounded-full object-cover shadow-elev"
          loading="lazy"
          onError={() => setImgError(true)}
        />
      ) : (
        <div
          className="flex h-24 w-24 items-center justify-center rounded-full shadow-elev"
          style={{
            background: `linear-gradient(135deg, hsl(${hue}, 30%, 25%), hsl(${(hue + 60) % 360}, 30%, 15%))`,
          }}
        >
          <span className="text-2xl font-bold text-white/40">
            {artist.name.charAt(0).toUpperCase()}
          </span>
        </div>
      )}
      <div className="mt-3 truncate text-sm font-bold text-ink-base">{artist.name}</div>
      <div className="text-xs text-ink-muted">
        {artist.primaryTrackCount ?? artist.trackCount} tracks
      </div>
    </button>
  );
}
