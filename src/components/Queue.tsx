import { usePlayerStore } from '../store/player';
import { useUIStore } from '../store/ui';
import { AlbumArt } from './AlbumArt';
import { formatDuration } from '../lib/format';

export function Queue() {
  const queueOpen = useUIStore((s) => s.queueOpen);
  const setQueueOpen = useUIStore((s) => s.setQueueOpen);
  const { queue, currentIndex, removeFromQueue } = usePlayerStore();

  if (!queueOpen) return null;

  const now = queue[currentIndex];
  const upcoming = queue.slice(currentIndex + 1);

  return (
    <aside className="flex h-full w-80 shrink-0 flex-col rounded-lg bg-surface">
      <header className="flex items-center justify-between px-4 py-4">
        <h2 className="font-title text-base font-bold text-ink-base">Queue</h2>
        <button
          onClick={() => setQueueOpen(false)}
          className="rounded-pill bg-surface-mid px-3 py-1 text-xs text-ink-muted hover:text-ink-base"
        >
          Close
        </button>
      </header>

      <div className="min-h-0 flex-1 overflow-y-auto px-2 pb-4">
        {queue.length === 0 && (
          <div className="px-3 py-8 text-center text-xs text-ink-faint">
            Queue is empty.
          </div>
        )}

        {now && (
          <>
            <div className="px-2 pb-2 pt-1 text-xs font-bold uppercase tracking-button text-ink-muted">
              Now playing
            </div>
            <QueueRow track={now} isCurrent onRemove={null} />
          </>
        )}

        {upcoming.length > 0 && (
          <>
            <div className="px-2 pb-2 pt-4 text-xs font-bold uppercase tracking-button text-ink-muted">
              Next up
            </div>
            {upcoming.map((t, i) => (
              <QueueRow
                key={`${t.id}-${i}`}
                track={t}
                onRemove={() => removeFromQueue(currentIndex + 1 + i)}
              />
            ))}
          </>
        )}
      </div>
    </aside>
  );
}

function QueueRow({
  track,
  isCurrent = false,
  onRemove,
}: {
  track: import('../types').Track;
  isCurrent?: boolean;
  onRemove: (() => void) | null;
}) {
  return (
    <div className="group flex items-center gap-3 rounded-md p-2 hover:bg-surface-mid">
      <AlbumArt trackId={track.id} size={40} />
      <div className="min-w-0 flex-1">
        <div className={`truncate text-sm font-bold ${isCurrent ? 'text-brand' : 'text-ink-base'}`}>
          {track.title}
        </div>
        <div className="truncate text-xs text-ink-muted">{track.artist ?? 'Unknown'}</div>
      </div>
      <span className="text-xs tabular-nums text-ink-muted">{formatDuration(track.duration)}</span>
      {onRemove && (
        <button
          onClick={onRemove}
          className="opacity-0 transition-opacity group-hover:opacity-100"
          aria-label="Remove from queue"
        >
          <span className="text-ink-muted hover:text-ink-base">✕</span>
        </button>
      )}
    </div>
  );
}
