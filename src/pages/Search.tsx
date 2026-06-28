import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { api } from '../lib/api';
import type { SearchResult } from '../types';
import { TrackList } from '../components/TrackList';
import { AlbumGrid } from '../components/AlbumGrid';
import { SearchBar } from '../components/SearchBar';

export function SearchPage() {
  const navigate = useNavigate();
  const [query, setQuery] = useState('');
  const [results, setResults] = useState<SearchResult | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    const q = query.trim();
    if (!q) {
      setResults(null);
      setLoading(false);
      return;
    }
    setLoading(true);
    // guard against stale responses: if the user types "a" then "ab", the "a"
    // response must not overwrite the "ab" results, and must not clear the
    // spinner while "ab" is still in flight.
    let cancelled = false;
    const t = setTimeout(() => {
      api
        .search(q)
        .then((r) => {
          if (cancelled) return;
          setResults(r);
          setLoading(false);
        })
        .catch(() => {
          if (cancelled) return;
          setLoading(false);
        });
    }, 250);
    return () => {
      cancelled = true;
      clearTimeout(t);
    };
  }, [query]);

  return (
    <div className="space-y-8 p-6">
      <SearchBar value={query} onChange={setQuery} autoFocus placeholder="Songs, artists, albums…" />

      {!query && (
        <div className="py-20 text-center text-sm text-ink-faint">
          Start typing to search your local library.
        </div>
      )}

      {loading && (
        <div className="py-12 text-center text-sm text-ink-faint">Searching…</div>
      )}

      {!loading && results && (
        <>
          {/* artists */}
          {results.artists.length > 0 && (
            <section>
              <h2 className="mb-3 font-title text-lg font-bold text-ink-base">Artists</h2>
              <div className="grid grid-cols-2 gap-4 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-6">
                {results.artists.slice(0, 12).map((a) => (
                  <button
                    key={a.id}
                    onClick={() => navigate(`/artist/${a.id}`)}
                    className="group card flex flex-col items-center text-center"
                  >
                    <div
                      className="flex h-24 w-24 items-center justify-center rounded-full shadow-elev"
                      style={{
                        background: `linear-gradient(135deg, hsl(${(a.id * 67) % 360}, 30%, 25%), hsl(${(a.id * 67 + 60) % 360}, 30%, 15%))`,
                      }}
                    >
                      <span className="text-2xl font-bold text-white/40">
                        {a.name.charAt(0).toUpperCase()}
                      </span>
                    </div>
                    <div className="mt-3 truncate text-sm font-bold text-ink-base">{a.name}</div>
                    <div className="text-xs text-ink-muted">Artist</div>
                  </button>
                ))}
              </div>
            </section>
          )}

          {/* albums */}
          {results.albums.length > 0 && (
            <section>
              <h2 className="mb-3 font-title text-lg font-bold text-ink-base">Albums</h2>
              <AlbumGrid albums={results.albums.slice(0, 10)} />
            </section>
          )}

          {/* tracks */}
          <section>
            <h2 className="mb-3 font-title text-lg font-bold text-ink-base">Songs</h2>
            <TrackList tracks={results.tracks} emptyMessage="No matches." />
          </section>
        </>
      )}
    </div>
  );
}
