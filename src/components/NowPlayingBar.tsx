import { useRef } from 'react';
import { usePlayerStore } from '../store/player';
import { useLibraryStore } from '../store/library';
import { useUIStore } from '../store/ui';
import { api } from '../lib/api';
import { formatDuration } from '../lib/format';
import { AlbumArt } from './AlbumArt';
import { ArtistLinks } from './ArtistLinks';
import {
  ChevronDownIcon,
  HeartFilledIcon,
  HeartIcon,
  MuteIcon,
  NextIcon,
  PauseIcon,
  PlayIcon,
  PrevIcon,
  QueueIcon,
  RepeatIcon,
  RepeatOneIcon,
  ShuffleIcon,
  VolumeIcon,
} from './icons';

export function NowPlayingBar() {
  const {
    queue,
    currentIndex,
    isPlaying,
    currentTime,
    duration,
    volume,
    muted,
    shuffle,
    repeat,
    togglePlay,
    next,
    prev,
    setVolume,
    toggleMute,
    toggleShuffle,
    cycleRepeat,
  } = usePlayerStore();

  const toggleQueue = useUIStore((s) => s.toggleQueue);
  const queueOpen = useUIStore((s) => s.queueOpen);
  const togglePanel = useUIStore((s) => s.toggleNowPlayingPanel);
  const panelOpen = useUIStore((s) => s.nowPlayingPanelOpen);
  const track = queue[currentIndex];

  const seekRef = useRef<HTMLInputElement | null>(null);

  const handleSeek = (e: React.ChangeEvent<HTMLInputElement>) => {
    const t = Number(e.target.value);
    const st = usePlayerStore.getState() as unknown as { _seekAudio?: (t: number) => void };
    st._seekAudio?.(t);
  };

  const progressPct = duration > 0 ? Math.min(100, (currentTime / duration) * 100) : 0;

  return (
    <footer className="flex h-[88px] shrink-0 items-center justify-between gap-4 bg-base px-4">
      {/* left: track info */}
      <div className="flex min-w-0 flex-1 items-center gap-3">
        {track ? (
          <>
            <button
              onClick={togglePanel}
              className="relative shrink-0 rounded-md transition-opacity hover:opacity-80"
              aria-label="Open now playing panel"
              title="Now playing"
            >
              <AlbumArt trackId={track.id} size={56} />
              <span className="absolute inset-0 flex items-center justify-center rounded-md bg-black/40 opacity-0 transition-opacity hover:opacity-100">
                <ChevronDownIcon width={20} height={20} className="text-white" />
              </span>
            </button>
            <div className="min-w-0">
              <div className="truncate text-sm font-bold text-ink-base">{track.title}</div>
              <div className="truncate text-xs text-ink-muted">
                <ArtistLinks raw={track.artist} artistId={track.artistId} artists={track.artists} />
              </div>
            </div>
            <LikeButton trackId={track.id} liked={track.liked} />
          </>
        ) : (
          <div className="text-sm text-ink-faint">Nothing playing</div>
        )}
      </div>

      {/* center: transport + progress */}
      <div className="flex flex-[2] flex-col items-center gap-2">
        <div className="flex items-center gap-5">
          <button
            onClick={toggleShuffle}
            className={`transition-colors ${shuffle ? 'text-brand' : 'text-ink-muted hover:text-ink-base'}`}
            aria-label="Shuffle"
            title="Shuffle"
          >
            <ShuffleIcon width={18} height={18} />
          </button>
          <button
            onClick={prev}
            className="text-ink-muted transition-colors hover:text-ink-base"
            aria-label="Previous"
          >
            <PrevIcon width={22} height={22} />
          </button>
          <button
            onClick={togglePlay}
            className="flex h-9 w-9 items-center justify-center rounded-full bg-ink-base text-base hover:scale-105"
            aria-label={isPlaying ? 'Pause' : 'Play'}
          >
            {isPlaying ? <PauseIcon width={20} height={20} /> : <PlayIcon width={20} height={20} />}
          </button>
          <button
            onClick={next}
            className="text-ink-muted transition-colors hover:text-ink-base"
            aria-label="Next"
          >
            <NextIcon width={22} height={22} />
          </button>
          <button
            onClick={cycleRepeat}
            className={`transition-colors ${repeat !== 'off' ? 'text-brand' : 'text-ink-muted hover:text-ink-base'}`}
            aria-label="Repeat"
            title={`Repeat: ${repeat}`}
          >
            {repeat === 'one' ? (
              <RepeatOneIcon width={18} height={18} />
            ) : (
              <RepeatIcon width={18} height={18} />
            )}
          </button>
        </div>
        <div className="flex w-full max-w-xl items-center gap-2">
          <span className="w-10 text-right text-[11px] tabular-nums text-ink-muted">
            {formatDuration(currentTime)}
          </span>
          <input
            ref={seekRef}
            type="range"
            min={0}
            max={duration || 0}
            step={0.1}
            value={currentTime}
            onChange={handleSeek}
            className="group relative h-1 flex-1 cursor-pointer appearance-none rounded-full bg-edge/40
              [&::-webkit-slider-thumb]:h-3 [&::-webkit-slider-thumb]:w-3 [&::-webkit-slider-thumb]:appearance-none
              [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-ink-base
              [&::-moz-range-thumb]:h-3 [&::-moz-range-thumb]:w-3 [&::-moz-range-thumb]:rounded-full [&::-moz-range-thumb]:bg-ink-base [&::-moz-range-thumb]:border-0"
            style={{
              background: `linear-gradient(to right, #1ed760 ${progressPct}%, rgba(77,77,77,0.4) ${progressPct}%)`,
            }}
          />
          <span className="w-10 text-[11px] tabular-nums text-ink-muted">
            {formatDuration(duration)}
          </span>
        </div>
      </div>

      {/* right: volume + queue toggle */}
      <div className="flex flex-1 items-center justify-end gap-3">
        <button
          onClick={toggleQueue}
          className={`rounded-md p-2 transition-colors ${
            queueOpen ? 'text-brand' : 'text-ink-muted hover:text-ink-base'
          }`}
          aria-label="Toggle queue"
          title="Queue"
        >
          <QueueIcon width={18} height={18} />
        </button>
        <button
          onClick={toggleMute}
          className="text-ink-muted transition-colors hover:text-ink-base"
          aria-label="Mute"
        >
          {muted || volume === 0 ? (
            <MuteIcon width={18} height={18} />
          ) : (
            <VolumeIcon width={18} height={18} />
          )}
        </button>
        <input
          type="range"
          min={0}
          max={1}
          step={0.01}
          value={muted ? 0 : volume}
          onChange={(e) => setVolume(Number(e.target.value))}
          className="h-1 w-24 cursor-pointer appearance-none rounded-full bg-edge/40
            [&::-webkit-slider-thumb]:h-3 [&::-webkit-slider-thumb]:w-3 [&::-webkit-slider-thumb]:appearance-none
            [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-ink-base
            [&::-moz-range-thumb]:h-3 [&::-moz-range-thumb]:w-3 [&::-moz-range-thumb]:rounded-full [&::-moz-range-thumb]:bg-ink-base [&::-moz-range-thumb]:border-0"
          style={{
            background: `linear-gradient(to right, #ffffff ${(muted ? 0 : volume) * 100}%, rgba(77,77,77,0.4) ${(muted ? 0 : volume) * 100}%)`,
          }}
        />
      </div>
    </footer>
  );
}

function LikeButton({ trackId, liked }: { trackId: number; liked: boolean }) {
  const toggleLikeOptimistic = useLibraryStore((s) => s.toggleLikeOptimistic);
  const handleLike = async () => {
    // Optimistic update across BOTH stores so the heart stays consistent between
    // the NowPlayingBar, TrackRow, and the Liked/Library pages.
    toggleLikeOptimistic(trackId);
    usePlayerStore.setState((s) => ({
      queue: s.queue.map((t) => (t.id === trackId ? { ...t, liked: !liked } : t)),
    }));
    try {
      await api.toggleLike(trackId, !liked);
    } catch {
      // rollback both stores on failure
      toggleLikeOptimistic(trackId);
      usePlayerStore.setState((s) => ({
        queue: s.queue.map((t) => (t.id === trackId ? { ...t, liked } : t)),
      }));
    }
  };
  return (
    <button
      onClick={handleLike}
      className={`ml-2 ${liked ? 'text-brand' : 'text-ink-muted hover:text-ink-base'}`}
      aria-label={liked ? 'Unlike' : 'Like'}
    >
      {liked ? <HeartFilledIcon width={16} height={16} /> : <HeartIcon width={16} height={16} />}
    </button>
  );
}
