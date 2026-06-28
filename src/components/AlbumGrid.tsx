import { useNavigate } from 'react-router-dom';
import type { Album } from '../types';
import { API_ORIGIN } from '../lib/api';

interface AlbumGridProps {
  albums: Album[];
  emptyMessage?: string;
}

export function AlbumGrid({ albums, emptyMessage = 'No albums.' }: AlbumGridProps) {
  const navigate = useNavigate();
  if (albums.length === 0) {
    return <div className="py-12 text-center text-sm text-ink-faint">{emptyMessage}</div>;
  }
  return (
    <div className="grid grid-cols-2 gap-4 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5">
      {albums.map((a) => (
        <button
          key={a.id}
          onClick={() => navigate(`/album/${a.id}`)}
          className="group card text-left"
        >
          <AlbumCover album={a} />
          <div className="mt-3 truncate text-sm font-bold text-ink-base">{a.name}</div>
          <div className="mt-1 truncate text-xs text-ink-muted">
            {a.albumArtist ?? 'Unknown artist'}
          </div>
        </button>
      ))}
    </div>
  );
}

function AlbumCover({ album }: { album: Album }) {
  const hue = (album.id * 67) % 360;
  const placeholder = {
    background: album.hasCover
      ? `hsl(${hue}, 30%, 30%)`
      : `linear-gradient(135deg, hsl(${hue}, 25%, 25%), hsl(${(hue + 60) % 360}, 25%, 18%))`,
  };

  if (!album.coverTrackId) {
    return <div className="aspect-square w-full rounded-md shadow-elev" style={placeholder} />;
  }

  return (
    <div className="aspect-square w-full overflow-hidden rounded-md shadow-elev">
      <img
        src={`${API_ORIGIN}/api/cover/${album.coverTrackId}`}
        alt=""
        className="h-full w-full object-cover"
        loading="lazy"
        onError={(e) => {
          const target = e.currentTarget;
          target.style.display = 'none';
          target.parentElement?.classList.add('cover-fallback');
          // ensure the parent still shows a placeholder when the image is hidden
          if (target.parentElement) {
            target.parentElement.style.background = placeholder.background;
          }
        }}
      />
    </div>
  );
}
