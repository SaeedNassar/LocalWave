import { useEffect, useState } from 'react';
import { api } from '../lib/api';
import { MusicNoteIcon } from './icons';

interface AlbumArtProps {
  trackId?: number | null;
  size?: number;
  rounded?: string;
  className?: string;
}

export function AlbumArt({ trackId, size = 48, rounded = 'rounded-md', className = '' }: AlbumArtProps) {
  const [errored, setErrored] = useState(false);
  // Reset the error flag when the track changes, otherwise a single 404 makes
  // every subsequent track fall back to the note icon until remount.
  useEffect(() => {
    setErrored(false);
  }, [trackId]);
  const showArt = trackId && !errored;
  const px = `${size}px`;
  return (
    <div
      className={`flex items-center justify-center bg-surface-card text-ink-faint overflow-hidden ${rounded} ${className}`}
      style={{ width: px, height: px, minWidth: px, minHeight: px }}
    >
      {showArt ? (
        <img
          src={api.coverUrl(trackId)}
          alt=""
          width={size}
          height={size}
          onError={() => setErrored(true)}
          className="h-full w-full object-cover"
          loading="lazy"
        />
      ) : (
        <MusicNoteIcon width={size * 0.4} height={size * 0.4} />
      )}
    </div>
  );
}
