import { useEffect, useState } from 'react';
import { api, API_ORIGIN } from '../lib/api';
import type { ScanProgress, ScanStatus } from '../types';

interface Settings {
  musicFolder: string;
  supportedExtensions: string[];
  scanIntervalMs: number;
  port: number;
  spDc: string;
  enableLyrics: boolean;
  enableCanvas: boolean;
  musixmatchAccessToken: string;
}

export function SettingsPage() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [status, setStatus] = useState<ScanStatus | null>(null);
  const [folderInput, setFolderInput] = useState('');
  const [exts, setExts] = useState('');
  const [spDc, setSpDc] = useState('');
  const [enableLyrics, setEnableLyrics] = useState(true);
  const [enableCanvas, setEnableCanvas] = useState(false);
  const [musixmatchAccessToken, setMusixmatchAccessToken] = useState('');
  const [saving, setSaving] = useState(false);
  const [scanning, setScanning] = useState(false);
  const [lastScan, setLastScan] = useState<ScanProgress | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);

  const load = async () => {
    try {
      const s = await fetch(`${API_ORIGIN}/api/settings`).then((r) => r.json());
      setSettings(s);
      setFolderInput(s.musicFolder);
      setExts(s.supportedExtensions.join(', '));
      setSpDc(s.spDc ?? '');
      setEnableLyrics(s.enableLyrics ?? true);
      setEnableCanvas(s.enableCanvas ?? false);
      setMusixmatchAccessToken(s.musixmatchAccessToken ?? '');
      const st = await api.getScanStatus();
      setStatus(st);
      setError(null);
    } catch (err) {
      setError((err as Error).message);
    }
  };

  useEffect(() => {
    load();
  }, []);

  const handleSave = async () => {
    setSaving(true);
    setError(null);
    setSuccess(null);
    try {
      const parsedExts = exts
        .split(',')
        .map((e) => e.trim().toLowerCase())
        .filter(Boolean)
        .map((e) => (e.startsWith('.') ? e : '.' + e));
      const res = await fetch(`${API_ORIGIN}/api/settings`, {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          musicFolder: folderInput,
          supportedExtensions: parsedExts,
          spDc,
          enableLyrics,
          enableCanvas,
          musixmatchAccessToken,
        }),
      });
      if (!res.ok) {
        const body = await res.json();
        throw new Error(body.error ?? 'Failed to save');
      }
      const folderChanged = folderInput !== settings?.musicFolder;
      setSuccess(
        folderChanged
          ? 'Settings saved. Restart the server for the new music folder to take effect.'
          : 'Settings saved.',
      );
      await load();
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setSaving(false);
    }
  };

  const handleRescan = async () => {
    setScanning(true);
    setError(null);
    try {
      const progress = await api.rescan();
      setLastScan(progress);
      await load();
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setScanning(false);
    }
  };

  if (!settings) {
    if (error) return <div className="p-6 text-sm text-semantic-neg">{error}</div>;
    return <div className="p-6 text-sm text-ink-faint">Loading settings…</div>;
  }

  return (
    <div className="mx-auto max-w-2xl space-y-8 p-6">
      <header>
        <h1 className="font-title text-2xl font-bold text-ink-base">Settings</h1>
        <p className="mt-1 text-sm text-ink-muted">Configure your LocalWave library.</p>
      </header>

      {status && (
        <div className="card">
          <h2 className="mb-2 font-title text-base font-bold text-ink-base">Library status</h2>
          <div className="grid grid-cols-2 gap-3 text-sm sm:grid-cols-4">
            <Stat label="Tracks" value={status.tracks} />
            <Stat label="Albums" value={status.albums} />
            <Stat label="Artists" value={status.artists} />
            <Stat label="Playlists" value={status.playlists} />
          </div>
          <button onClick={handleRescan} disabled={scanning} className="btn-pill mt-4">
            {scanning ? 'Scanning…' : 'Rescan library'}
          </button>
          {lastScan && (
            <p className="mt-2 text-xs text-ink-muted">
              Last scan: {lastScan.added} added, {lastScan.updated} updated, {lastScan.failed} failed.
            </p>
          )}
        </div>
      )}

      <div className="card space-y-4">
        <h2 className="font-title text-base font-bold text-ink-base">Enrichment</h2>

        <ToggleRow
          label="Lyrics"
          description="Fetch synced lyrics from Musixmatch (if token is provided) or fall back to lrclib.net, and cache them in the database."
          checked={enableLyrics}
          onChange={setEnableLyrics}
        />

        <label className="block">
          <span className="text-xs font-bold uppercase tracking-button text-ink-muted">
            Musixmatch Access Token
          </span>
          <input
            value={musixmatchAccessToken}
            onChange={(e) => setMusixmatchAccessToken(e.target.value)}
            className="mt-1 w-full rounded-md bg-surface-mid px-3 py-2 text-sm text-ink-base focus:outline-none focus:ring-1 focus:ring-brand"
            spellCheck={false}
            placeholder="Paste Musixmatch access token here…"
            type="password"
          />
          <span className="mt-1 block text-xs text-ink-faint">
            Optional primary source for synced lyrics. lrclib.net is used as a fallback.
          </span>
        </label>

        <ToggleRow
          label="Spotify Canvas"
          description="Fetch looping Canvas music videos from Spotify (requires sp_dc cookie below)."
          checked={enableCanvas}
          onChange={setEnableCanvas}
        />

        <label className="block">
          <span className="text-xs font-bold uppercase tracking-button text-ink-muted">
            Spotify sp_dc cookie
          </span>
          <input
            value={spDc}
            onChange={(e) => setSpDc(e.target.value)}
            className="mt-1 w-full rounded-md bg-surface-mid px-3 py-2 text-sm text-ink-base focus:outline-none focus:ring-1 focus:ring-brand"
            spellCheck={false}
            placeholder="Paste sp_dc cookie value here…"
            type="password"
          />
          <span className="mt-1 block text-xs text-ink-faint">
            Open Spotify Web Player → DevTools → Application → Cookies → copy sp_dc value.
            Needed for Canvas videos. Artist images work without it.
          </span>
        </label>
      </div>

      <div className="card space-y-4">
        <h2 className="font-title text-base font-bold text-ink-base">Music folder</h2>
        <label className="block">
          <span className="text-xs font-bold uppercase tracking-button text-ink-muted">
            Root folder path
          </span>
          <input
            value={folderInput}
            onChange={(e) => setFolderInput(e.target.value)}
            className="mt-1 w-full rounded-md bg-surface-mid px-3 py-2 text-sm text-ink-base focus:outline-none focus:ring-1 focus:ring-brand"
            spellCheck={false}
          />
        </label>
        <label className="block">
          <span className="text-xs font-bold uppercase tracking-button text-ink-muted">
            Supported extensions (comma-separated)
          </span>
          <input
            value={exts}
            onChange={(e) => setExts(e.target.value)}
            className="mt-1 w-full rounded-md bg-surface-mid px-3 py-2 text-sm text-ink-base focus:outline-none focus:ring-1 focus:ring-brand"
            spellCheck={false}
          />
        </label>
        <div className="flex items-center gap-3">
          <button onClick={handleSave} disabled={saving} className="btn-brand">
            {saving ? 'Saving…' : 'Save settings'}
          </button>
          {error && <span className="text-sm text-semantic-neg">{error}</span>}
          {success && <span className="text-sm text-brand">{success}</span>}
        </div>
      </div>

      <div className="card">
        <h2 className="mb-2 font-title text-base font-bold text-ink-base">Backend</h2>
        <p className="text-xs text-ink-muted">
          API server running on port <span className="font-bold text-ink-base">{settings.port}</span>.
          Vite dev server on port 5174.
        </p>
      </div>
    </div>
  );
}

function Stat({ label, value }: { label: string; value: number }) {
  return (
    <div className="rounded-md bg-surface-mid px-3 py-2">
      <div className="text-2xl font-bold text-ink-base">{value}</div>
      <div className="text-xs text-ink-muted">{label}</div>
    </div>
  );
}

function ToggleRow({
  label,
  description,
  checked,
  onChange,
}: {
  label: string;
  description: string;
  checked: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <div className="flex items-start justify-between gap-4">
      <div>
        <div className="text-sm font-bold text-ink-base">{label}</div>
        <div className="text-xs text-ink-muted">{description}</div>
      </div>
      <button
        onClick={() => onChange(!checked)}
        className={`relative h-6 w-11 shrink-0 rounded-full transition-colors ${
          checked ? 'bg-brand' : 'bg-surface-card'
        }`}
        role="switch"
        aria-checked={checked}
        aria-label={label}
      >
        <span
          className={`absolute top-0.5 h-5 w-5 rounded-full bg-white transition-transform ${
            checked ? 'translate-x-[22px]' : 'translate-x-0.5'
          }`}
        />
      </button>
    </div>
  );
}
