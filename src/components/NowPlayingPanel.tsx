import { useEffect, useRef, useState } from 'react';
import { usePlayerStore } from '../store/player';
import { api } from '../lib/api';
import type { LyricsResult, CanvasResult, ArtistWithRole, LyricLine } from '../types';
import { formatDuration } from '../lib/format';
import { AlbumArt } from './AlbumArt';
import { ArtistLinks } from './ArtistLinks';

export function NowPlayingPanel() {
  const { queue, currentIndex, currentTime } = usePlayerStore();
  const track = queue[currentIndex];

  const [lyrics, setLyrics] = useState<LyricsResult | null>(null);
  const [canvas, setCanvas] = useState<CanvasResult | null>(null);
  const [credits, setCredits] = useState<ArtistWithRole[]>([]);

  // These refs + effects MUST be declared before any early return, otherwise the
  // number of hooks changes between renders (Rules of Hooks violation) and React
  // throws when the queue transitions from non-empty to empty.
  const lyricsRef = useRef<HTMLDivElement>(null);
  const lineRefs = useRef<(HTMLElement | null)[]>([]);

  useEffect(() => {
    setLyrics(null);
    setCanvas(null);
    setCredits([]);
    if (!track) return;

    let cancelled = false;

    api.getLyrics(track.id).then((r) => !cancelled && setLyrics(r)).catch(() => {});
    api.getCanvas(track.id).then((r) => !cancelled && setCanvas(r)).catch(() => {});
    api.getTrackArtists(track.id).then((r) => !cancelled && setCredits(r.items)).catch(() => {});

    return () => { cancelled = true; };
  }, [track?.id]);

  const activeLineIndex =
    track && lyrics?.synced ? findActiveLine(lyrics.lines, currentTime) : -1;

  useEffect(() => {
    if (!lyrics?.synced || activeLineIndex < 0) return;
    const container = lyricsRef.current;
    const activeLine = lineRefs.current[activeLineIndex];
    if (!container || !activeLine) return;

    const containerHeight = container.clientHeight;
    const lineTop = activeLine.offsetTop;
    const lineHeight = activeLine.clientHeight;
    const target = lineTop - containerHeight / 2 + lineHeight / 2;

    container.scrollTo({ top: Math.max(0, target), behavior: 'smooth' });
  }, [activeLineIndex, lyrics?.synced]);

  if (!track) return null;

  const upcoming = queue.slice(currentIndex + 1);

  const hasCanvas = !!canvas?.url;

  const seekAudio = (time: number) => {
    const st = usePlayerStore.getState() as unknown as { _seekAudio?: (t: number) => void };
    st._seekAudio?.(time);
  };

  return (
    <div className="relative flex h-full w-full flex-col overflow-y-auto">
      {/* ── Canvas background in the top hero area only ─────── */}
      {hasCanvas && (
        <div className="pointer-events-none absolute inset-x-0 top-0 z-0 h-[55%] overflow-hidden">
          <video
            key={canvas.url}
            src={canvas.url}
            poster={api.coverUrl(track.id)}
            autoPlay
            loop
            muted
            playsInline
            className="h-full w-full object-cover"
          />
          {/* fade into the dark body of the panel */}
          <div className="absolute inset-0 bg-gradient-to-b from-black/50 via-black/30 to-[#121212]" />

          {/* Title + artist float at the bottom of the canvas area */}
          <div className="absolute inset-x-0 bottom-0 p-4 pb-3 text-center">
            <h2 className="font-title text-xl font-bold text-white drop-shadow">
              {track.title}
            </h2>
            <p className="mt-1 text-sm text-white/80 drop-shadow">
              <ArtistLinks
                raw={track.artist}
                artistId={track.artistId}
                artists={track.artists}
                linkClassName="text-white/80"
              />
            </p>
          </div>
        </div>
      )}

      {/* Spacer that matches the canvas height so content starts below it */}
      {hasCanvas && <div className="h-[55%] w-full shrink-0" />}

      {/* ── Content ─────────────────────────────────────────── */}
      <div className="relative z-10 flex flex-col gap-4 p-4">
        {/* ── Album art + Title + artist (no canvas) ───────────── */}
        {!hasCanvas && (
          <div className="flex flex-col gap-4">
            <div className="aspect-square w-full overflow-hidden rounded-lg bg-surface-card shadow-dialog">
              <img
                src={api.coverUrl(track.id)}
                alt=""
                className="h-full w-full object-cover"
              />
            </div>

            <div>
              <h2 className="font-title text-xl font-bold text-ink-base">
                {track.title}
              </h2>
              <p className="mt-1 text-sm text-ink-muted">
                <ArtistLinks
                  raw={track.artist}
                  artistId={track.artistId}
                  artists={track.artists}
                />
              </p>
            </div>
          </div>
        )}

        {/* ── Next in queue ─────────────────────────────────── */}
        {upcoming.length > 0 && (
          <div className={`rounded-md p-3 ${hasCanvas ? 'bg-black/40 backdrop-blur-sm' : 'bg-surface-mid'}`}>
            <div className={`mb-2 text-xs font-bold uppercase tracking-button ${hasCanvas ? 'text-white/70' : 'text-ink-muted'}`}>
              Next in queue
            </div>
            <div className="flex items-center gap-3">
              <AlbumArt trackId={upcoming[0].id} size={40} />
              <div className="min-w-0 flex-1">
                <div className={`truncate text-sm font-bold ${hasCanvas ? 'text-white' : 'text-ink-base'}`}>
                  {upcoming[0].title}
                </div>
                <div className={`truncate text-xs ${hasCanvas ? 'text-white/70' : 'text-ink-muted'}`}>
                  {upcoming[0].artist ?? 'Unknown'}
                </div>
              </div>
              <span className={`text-xs tabular-nums ${hasCanvas ? 'text-white/70' : 'text-ink-muted'}`}>
                {formatDuration(upcoming[0].duration)}
              </span>
            </div>
          </div>
        )}

        {/* ── Lyrics ───────────────────────────────────────── */}
        {lyrics?.synced && lyrics.lines.length > 0 ? (
          <div className={`rounded-md p-3 ${hasCanvas ? 'bg-black/40 backdrop-blur-sm' : 'bg-surface-mid/50'}`}>
            <div className={`mb-2 text-xs font-bold uppercase tracking-button ${hasCanvas ? 'text-white/70' : 'text-ink-muted'}`}>
              Lyrics
            </div>
            <div
              ref={lyricsRef}
              className="relative max-h-64 space-y-1 overflow-y-auto pr-1"
            >
              {lyrics.lines.map((line, i) => (
                <button
                  key={i}
                  ref={(el) => { lineRefs.current[i] = el; }}
                  onClick={() => seekAudio(line.time)}
                  className={`block w-full text-left text-sm transition-colors hover:opacity-80 ${
                    i === activeLineIndex
                      ? hasCanvas ? 'font-bold text-white' : 'font-bold text-ink-base'
                      : hasCanvas ? 'text-white/70' : 'text-ink-muted'
                  }`}
                >
                  {line.text || '…'}
                </button>
              ))}
            </div>
          </div>
        ) : lyrics?.lyrics ? (
          <div className={`rounded-md p-3 ${hasCanvas ? 'bg-black/40 backdrop-blur-sm' : 'bg-surface-mid/50'}`}>
            <div className={`mb-2 text-xs font-bold uppercase tracking-button ${hasCanvas ? 'text-white/70' : 'text-ink-muted'}`}>
              Lyrics
            </div>
            <div className="max-h-64 overflow-y-auto pr-1">
              <pre className={`whitespace-pre-wrap font-sans text-sm ${hasCanvas ? 'text-white/70' : 'text-ink-muted'}`}>
                {lyrics.lyrics}
              </pre>
            </div>
          </div>
        ) : null}

        {/* ── Credits ──────────────────────────────────────── */}
        {credits.length > 0 && (
          <div>
            <div className={`mb-2 text-xs font-bold uppercase tracking-button ${hasCanvas ? 'text-white/70' : 'text-ink-muted'}`}>
              Credits
            </div>
            <div className="flex flex-wrap gap-2">
              {credits.map((a) => (
                <CreditChip key={a.id} artist={a} hasCanvas={hasCanvas} />
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

function CreditChip({ artist, hasCanvas }: { artist: ArtistWithRole; hasCanvas: boolean }) {
  const [img, setImg] = useState<string | null>(artist.imagePath ?? null);

  useEffect(() => {
    if (artist.imagePath) { setImg(artist.imagePath); return; }
    let cancelled = false;
    api.getArtistImage(artist.id).then((r) => { if (!cancelled && r.imageUrl) setImg(r.imageUrl); }).catch(() => {});
    return () => { cancelled = true; };
  }, [artist.id, artist.imagePath]);

  return (
    <div className={`flex items-center gap-2 rounded-full py-1 pl-1 pr-3 ${hasCanvas ? 'bg-black/40 backdrop-blur-sm' : 'bg-surface-mid'}`}>
      {img ? (
        <img
          src={img}
          alt=""
          className="h-7 w-7 rounded-full object-cover"
          onError={() => setImg(null)}
        />
      ) : (
        <div className={`flex h-7 w-7 items-center justify-center rounded-full text-[10px] font-bold ${hasCanvas ? 'bg-white/10 text-white/80' : 'bg-surface-card text-ink-muted'}`}>
          {artist.name.charAt(0).toUpperCase()}
        </div>
      )}
      <span className={`text-xs font-bold ${hasCanvas ? 'text-white' : 'text-ink-base'}`}>{artist.name}</span>
      {artist.role !== 'primary' && (
        <span className={`text-[10px] uppercase ${hasCanvas ? 'text-white/60' : 'text-ink-faint'}`}>{artist.role}</span>
      )}
    </div>
  );
}

function findActiveLine(lines: LyricLine[], currentTime: number): number {
  let active = -1;
  for (let i = 0; i < lines.length; i++) {
    if (lines[i].time <= currentTime) active = i;
    else break;
  }
  return active;
}
