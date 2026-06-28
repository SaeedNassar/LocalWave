import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { usePlayerStore } from '../store/player';
import { useLibraryStore } from '../store/library';
import { api } from '../lib/api';
import { formatDuration } from '../lib/format';
import type { Track } from '../types';
import { AlbumArt } from './AlbumArt';
import { ArtistLinks } from './ArtistLinks';
import { TrackContextMenu } from './TrackContextMenu';
import { TrackMetadataEditor } from './TrackMetadataEditor';
import { HeartFilledIcon, HeartIcon, PauseIcon, PlayIcon } from './icons';

interface TrackRowProps {
  track: Track;
  index?: number;
  queue?: Track[];
  showArt?: boolean;
  showAlbum?: boolean;
  showLike?: boolean;
  draggable?: boolean;
  onDragStart?: (track: Track) => void;
}

export function TrackRow({
  track,
  index = 0,
  queue,
  showArt = true,
  showAlbum = true,
  showLike = true,
  draggable = true,
  onDragStart,
}: TrackRowProps) {
  const navigate = useNavigate();
  const { currentIndex, queue: playerQueue, isPlaying, playTrack, togglePlay } = usePlayerStore();
  const toggleLikeOptimistic = useLibraryStore((s) => s.toggleLikeOptimistic);
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number } | null>(null);
  const [editingMetadata, setEditingMetadata] = useState(false);

  const isCurrent = playerQueue[currentIndex]?.id === track.id;
  const isThisPlaying = isCurrent && isPlaying;

  const handlePlay = () => {
    if (isCurrent) {
      togglePlay();
    } else {
      playTrack(track, queue);
    }
  };

  const handleLike = async () => {
    toggleLikeOptimistic(track.id);
    try {
      await api.toggleLike(track.id, !track.liked);
    } catch {
      toggleLikeOptimistic(track.id); // rollback
    }
  };

  const handleContextMenu = (e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setContextMenu({ x: e.clientX, y: e.clientY });
  };

  return (
    <div
      className={`group grid grid-cols-[2rem_1fr_auto] items-center gap-3 rounded-md px-2 py-2 ${
        isCurrent ? 'bg-surface-mid/60' : 'hover:bg-surface-mid/40'
      }`}
      onDoubleClick={handlePlay}
      onContextMenu={handleContextMenu}
      draggable={draggable}
      onDragStart={(e) => {
        e.dataTransfer.setData('application/localwave-track', JSON.stringify({ trackId: track.id, title: track.title }));
        e.dataTransfer.effectAllowed = 'copy';
        onDragStart?.(track);
      }}
    >
      {/* index / play button */}
      <div className="flex w-8 items-center justify-center text-sm text-ink-muted">
        <span className={`group-hover:hidden ${isCurrent ? 'text-brand' : ''}`}>
          {isThisPlaying ? (
            <span className="font-bold text-brand">♪</span>
          ) : (
            index + 1
          )}
        </span>
        <button
          onClick={handlePlay}
          className="hidden text-ink-base group-hover:block"
          aria-label={isThisPlaying ? 'Pause' : 'Play'}
        >
          {isThisPlaying ? <PauseIcon /> : <PlayIcon />}
        </button>
      </div>

      {/* title + artist */}
      <div className="flex min-w-0 items-center gap-3">
        {showArt && <AlbumArt trackId={track.id} size={36} />}
        <div className="min-w-0 flex-1">
          <div
            className={`truncate text-sm font-bold ${
              isCurrent ? 'text-brand' : 'text-ink-base'
            }`}
          >
            {track.title}
          </div>
          <div className="truncate text-xs text-ink-muted">
            <ArtistLinks raw={track.artist} artistId={track.artistId} artists={track.artists} />
          </div>
        </div>
        {showAlbum && (
          <button
            onClick={() => track.albumId && navigate(`/album/${track.albumId}`)}
            className="hidden truncate text-xs text-ink-muted hover:underline md:block md:max-w-[12rem]"
          >
            {track.album ?? ''}
          </button>
        )}
        {showLike && (
          <button
            onClick={handleLike}
            className={`opacity-0 transition-opacity group-hover:opacity-100 ${
              track.liked ? 'text-brand' : 'text-ink-muted hover:text-ink-base'
            } ${track.liked ? '!opacity-100' : ''}`}
            aria-label={track.liked ? 'Unlike' : 'Like'}
          >
            {track.liked ? <HeartFilledIcon /> : <HeartIcon />}
          </button>
        )}
      </div>

      {/* duration */}
      <div className="text-xs tabular-nums text-ink-muted">
        {formatDuration(track.duration)}
      </div>

      {contextMenu && (
        <TrackContextMenu
          track={track}
          x={contextMenu.x}
          y={contextMenu.y}
          onClose={() => setContextMenu(null)}
          onEditMetadata={() => setEditingMetadata(true)}
        />
      )}

      {editingMetadata && (
        <TrackMetadataEditor track={track} onClose={() => setEditingMetadata(false)} />
      )}
    </div>
  );
}
