export interface TrackArtist {
  id: number;
  name: string;
  role: string;
  position: number;
}

export interface Track {
  id: number;
  path: string;
  filename: string;
  title: string;
  artist: string | null;
  album: string | null;
  albumArtist: string | null;
  albumId: number | null;
  artistId: number | null;
  duration: number;
  trackNumber: number | null;
  diskNumber: number | null;
  format: string | null;
  bitrate: number | null;
  sampleRate: number | null;
  hasCover: boolean;
  liked: boolean;
  playCount: number;
  dateAdded: string;
  fileModifiedAt: string | null;
  artists: TrackArtist[];
}

export interface Album {
  id: number;
  name: string;
  albumArtist: string | null;
  artistId: number | null;
  year: number | null;
  trackCount: number;
  hasCover: boolean;
  coverTrackId: number | null;
}

export interface Artist {
  id: number;
  name: string;
  albumCount: number;
  trackCount: number;
  primaryTrackCount?: number;
  imagePath?: string | null;
}

export interface ArtistDetail {
  artist: {
    id: number;
    name: string;
    imagePath: string | null;
    createdAt: string;
  };
  albums: Album[];
  appearsOn: Album[];
  primaryTracks: Track[];
  featuredTracks: Track[];
  totalTrackCount: number;
}

export interface ArtistTrack extends Track {
  role: string;
}

export interface SearchResult {
  tracks: Track[];
  albums: Album[];
  artists: Artist[];
}

export interface Playlist {
  id: number;
  name: string;
  description: string | null;
  trackCount: number;
  createdAt: string;
  isImported: boolean;
  source: string | null;
  position: number | null;
}

export interface PlaylistEntry {
  playlistId: number;
  position: number;
  trackId: number | null;
  rawEntry: string;
  missing: boolean;
  title: string | null;
  track: Track | null;
}

export interface PlaylistDetail extends Playlist {
  entries: PlaylistEntry[];
}

export interface ScanStatus {
  tracks: number;
  albums: number;
  artists: number;
  playlists: number;
}

export interface ScanProgress {
  scanned: number;
  added: number;
  updated: number;
  failed: number;
}

export interface ImportResult {
  playlist: Playlist;
  totalEntries: number;
  matched: number;
  missing: number;
  warnings: string[];
}

// ── Enrichment types ────────────────────────────────────────

export interface LyricLine {
  time: number;
  text: string;
}

export interface LyricsResult {
  lyrics: string;
  synced: boolean;
  lines: LyricLine[];
}

export interface CanvasResult {
  url: string;
  artistUri?: string | null;
  artistName?: string | null;
  artistImgUrl?: string | null;
}

export interface ArtistWithRole {
  id: number;
  name: string;
  imagePath: string | null;
  role: string;
}

export interface TrackMetadata {
  title: string;
  artist: string;
  album: string;
  albumArtist: string;
  year: number | null;
  trackNumber: number | null;
  coverArt: { mimeType: string; data: string } | null;
}

export interface TrackMetadataUpdate {
  title?: string;
  artist?: string;
  album?: string;
  albumArtist?: string;
  year?: number | null;
  trackNumber?: number | null;
  coverArt?: { mimeType: string; data: string } | null;
}

export interface ArtistImageResult {
  imageUrl: string | null;
  spotifyUrl: string | null;
}

export interface Features {
  lyrics: boolean;
  canvas: boolean;
  hasSpDc: boolean;
}
