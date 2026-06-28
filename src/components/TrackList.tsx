import type { Track } from '../types';
import { TrackRow } from './TrackRow';

interface TrackListProps {
  tracks: Track[];
  showHeader?: boolean;
  showArt?: boolean;
  showAlbum?: boolean;
  showLike?: boolean;
  emptyMessage?: string;
}

export function TrackList({
  tracks,
  showHeader = true,
  showArt = true,
  showAlbum = true,
  showLike = true,
  emptyMessage = 'No tracks.',
}: TrackListProps) {
  if (tracks.length === 0) {
    return <div className="py-12 text-center text-sm text-ink-faint">{emptyMessage}</div>;
  }

  return (
    <div>
      {showHeader && (
        <div className="grid grid-cols-[2rem_1fr_auto] items-center gap-3 border-b border-edge/40 px-2 pb-2 text-xs uppercase tracking-button text-ink-faint">
          <div className="text-center">#</div>
          <div>Title</div>
          <div>⏱</div>
        </div>
      )}
      <div className="py-2">
        {tracks.map((t, i) => (
          <TrackRow
            key={`${t.id}-${i}`}
            track={t}
            index={i}
            queue={tracks}
            showArt={showArt}
            showAlbum={showAlbum}
            showLike={showLike}
          />
        ))}
      </div>
    </div>
  );
}
