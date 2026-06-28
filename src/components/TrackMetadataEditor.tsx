import { useEffect, useState, useRef } from 'react';
import { api } from '../lib/api';
import { useLibraryStore } from '../store/library';
import type { Track, TrackMetadata, TrackMetadataUpdate } from '../types';

interface TrackMetadataEditorProps {
  track: Track;
  onClose: () => void;
}

export function TrackMetadataEditor({ track, onClose }: TrackMetadataEditorProps) {
  const [meta, setMeta] = useState<TrackMetadata | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [coverPreview, setCoverPreview] = useState<string | null>(null);
  const [coverRemoved, setCoverRemoved] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const loadAll = useLibraryStore((s) => s.loadAll);

  useEffect(() => {
    api.getTrackMetadata(track.id)
      .then((m) => {
        setMeta(m);
        if (m.coverArt) setCoverPreview(`data:${m.coverArt.mimeType};base64,${m.coverArt.data}`);
      })
      .finally(() => setLoading(false));
  }, [track.id]);

  const handleFileChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = () => {
      const result = reader.result as string;
      setCoverPreview(result);
      setCoverRemoved(false);
    };
    reader.readAsDataURL(file);
  };

  const handleSave = async () => {
    if (!meta) return;
    setSaving(true);
    try {
      let coverArt: { mimeType: string; data: string } | null | undefined;
      if (coverRemoved) {
        coverArt = null;
      } else if (coverPreview && coverPreview !== (meta.coverArt ? `data:${meta.coverArt.mimeType};base64,${meta.coverArt.data}` : null)) {
        const match = coverPreview.match(/^data:([^;]+);base64,(.+)$/);
        if (match) {
          coverArt = { mimeType: match[1], data: match[2] };
        }
      }

      const update: TrackMetadataUpdate = {
        title: meta.title,
        artist: meta.artist,
        album: meta.album,
        albumArtist: meta.albumArtist,
        year: meta.year,
        trackNumber: meta.trackNumber,
        coverArt,
      };

      await api.updateTrackMetadata(track.id, update);
      await loadAll();
      onClose();
    } catch (err) {
      alert((err as Error).message);
    } finally {
      setSaving(false);
    }
  };

  if (loading || !meta) {
    return (
      <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4">
        <div className="rounded-lg bg-surface p-6 text-sm text-ink-muted">Loading metadata…</div>
      </div>
    );
  }

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4"
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className="w-full max-w-md rounded-lg bg-surface p-6 shadow-dialog ring-1 ring-white/10">
        <h2 className="mb-4 font-title text-xl font-bold text-ink-base">Edit metadata</h2>

        <div className="space-y-3">
          <Field label="Title">
            <input
              value={meta.title}
              onChange={(e) => setMeta({ ...meta, title: e.target.value })}
              className="w-full rounded bg-surface-mid px-3 py-2 text-sm text-ink-base focus:outline-none focus:ring-1 focus:ring-brand"
            />
          </Field>
          <Field label="Artist">
            <input
              value={meta.artist}
              onChange={(e) => setMeta({ ...meta, artist: e.target.value })}
              className="w-full rounded bg-surface-mid px-3 py-2 text-sm text-ink-base focus:outline-none focus:ring-1 focus:ring-brand"
            />
          </Field>
          <Field label="Album">
            <input
              value={meta.album}
              onChange={(e) => setMeta({ ...meta, album: e.target.value })}
              className="w-full rounded bg-surface-mid px-3 py-2 text-sm text-ink-base focus:outline-none focus:ring-1 focus:ring-brand"
            />
          </Field>
          <Field label="Album artist">
            <input
              value={meta.albumArtist}
              onChange={(e) => setMeta({ ...meta, albumArtist: e.target.value })}
              className="w-full rounded bg-surface-mid px-3 py-2 text-sm text-ink-base focus:outline-none focus:ring-1 focus:ring-brand"
            />
          </Field>
          <div className="grid grid-cols-2 gap-3">
            <Field label="Year">
              <input
                type="number"
                value={meta.year ?? ''}
                onChange={(e) => setMeta({ ...meta, year: e.target.value ? Number(e.target.value) : null })}
                className="w-full rounded bg-surface-mid px-3 py-2 text-sm text-ink-base focus:outline-none focus:ring-1 focus:ring-brand"
              />
            </Field>
            <Field label="Track number">
              <input
                type="number"
                value={meta.trackNumber ?? ''}
                onChange={(e) => setMeta({ ...meta, trackNumber: e.target.value ? Number(e.target.value) : null })}
                className="w-full rounded bg-surface-mid px-3 py-2 text-sm text-ink-base focus:outline-none focus:ring-1 focus:ring-brand"
              />
            </Field>
          </div>

          <Field label="Cover art">
            <div className="flex items-center gap-3">
              {coverPreview ? (
                <img src={coverPreview} alt="" className="h-20 w-20 rounded-md object-cover" />
              ) : (
                <div className="flex h-20 w-20 items-center justify-center rounded-md bg-surface-mid text-xs text-ink-muted">
                  No cover
                </div>
              )}
              <div className="flex flex-col gap-2">
                <button
                  onClick={() => fileInputRef.current?.click()}
                  className="rounded bg-surface-mid px-3 py-1.5 text-xs font-bold text-ink-base hover:bg-surface-card"
                >
                  Replace
                </button>
                {(coverPreview || meta.coverArt) && (
                  <button
                    onClick={() => {
                      setCoverPreview(null);
                      setCoverRemoved(true);
                    }}
                    className="rounded bg-surface-mid px-3 py-1.5 text-xs font-bold text-semantic-neg hover:bg-surface-card"
                  >
                    Remove
                  </button>
                )}
              </div>
              <input
                ref={fileInputRef}
                type="file"
                accept="image/*"
                className="hidden"
                onChange={handleFileChange}
              />
            </div>
          </Field>
        </div>

        <div className="mt-6 flex justify-end gap-3">
          <button onClick={onClose} className="btn-pill">
            Cancel
          </button>
          <button onClick={handleSave} disabled={saving} className="btn-brand">
            {saving ? 'Saving…' : 'Save'}
          </button>
        </div>
      </div>
    </div>
  );
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <label className="block">
      <span className="mb-1 block text-xs font-bold uppercase tracking-button text-ink-muted">{label}</span>
      {children}
    </label>
  );
}
