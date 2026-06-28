import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { SearchIcon, PlusIcon } from './icons';
import { api } from '../lib/api';

interface SearchBarProps {
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
  autoFocus?: boolean;
}

export function SearchBar({ value, onChange, placeholder = 'Search your library', autoFocus }: SearchBarProps) {
  return (
    <div className="relative w-full max-w-md">
      <div className="pointer-events-none absolute left-4 top-1/2 -translate-y-1/2 text-ink-muted">
        <SearchIcon width={18} height={18} />
      </div>
      <input
        autoFocus={autoFocus}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        className="input-pill w-full pl-12 pr-4"
      />
    </div>
  );
}

export function ImportPlaylistButton({ onImported }: { onImported?: () => void }) {
  const navigate = useNavigate();
  const [busy, setBusy] = useState(false);

  const handleClick = () => {
    const filePath = window.prompt('Enter the absolute path to your .m3u8/.m3u file:');
    if (!filePath?.trim()) return;
    setBusy(true);
    api
      .importM3u(filePath.trim())
      .then((result) => {
        const msg =
          result.missing > 0
            ? `Imported "${result.playlist.name}" — ${result.matched}/${result.totalEntries} matched, ${result.missing} missing.`
            : `Imported "${result.playlist.name}" — all ${result.totalEntries} entries matched.`;
        window.alert(msg);
        onImported?.();
        navigate(`/playlist/${result.playlist.id}`);
      })
      .catch((err) => window.alert('Import failed: ' + (err as Error).message))
      .finally(() => setBusy(false));
  };

  return (
    <button onClick={handleClick} disabled={busy} className="btn-outlined">
      <PlusIcon width={14} height={14} />
      {busy ? 'Importing…' : 'Import .m3u8'}
    </button>
  );
}
