export function formatDuration(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) return '--:--';
  const total = Math.floor(seconds);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  if (h > 0) {
    return `${h}:${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`;
  }
  return `${m}:${String(s).padStart(2, '0')}`;
}

export function formatRelativeDate(iso: string): string {
  const d = new Date(iso);
  const now = new Date();
  const diffMs = now.getTime() - d.getTime();
  const day = 24 * 60 * 60 * 1000;
  if (diffMs < day) return 'today';
  if (diffMs < 2 * day) return 'yesterday';
  if (diffMs < 7 * day) return `${Math.floor(diffMs / day)} days ago`;
  return d.toLocaleDateString();
}
