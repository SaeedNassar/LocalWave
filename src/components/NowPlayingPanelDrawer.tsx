import { useUIStore } from '../store/ui';
import { NowPlayingPanel } from './NowPlayingPanel';
import { CloseIcon } from './icons';

/**
 * Right-side drawer that hosts the NowPlayingPanel.
 * Slides over the main content (and queue) when open.
 */
export function NowPlayingPanelDrawer() {
  const open = useUIStore((s) => s.nowPlayingPanelOpen);
  const setOpen = useUIStore((s) => s.setNowPlayingPanelOpen);

  if (!open) return null;

  return (
    <aside className="absolute inset-y-0 right-0 z-30 flex w-[340px] flex-col rounded-lg bg-surface shadow-dialog">
      <header className="flex items-center justify-between px-4 py-3">
        <h2 className="font-title text-sm font-bold text-ink-base">Now Playing</h2>
        <button
          onClick={() => setOpen(false)}
          className="rounded-md p-1 text-ink-muted transition-colors hover:bg-surface-mid hover:text-ink-base"
          aria-label="Close panel"
        >
          <CloseIcon width={18} height={18} />
        </button>
      </header>
      <div className="min-h-0 flex-1">
        <NowPlayingPanel />
      </div>
    </aside>
  );
}
