import { useMemo } from 'react';
import { useNavigate } from 'react-router-dom';
import type { TrackArtist } from '../types';

const DELIMS = /\s*;\s*|\s*\/\s*|\s*,\s*|\s+[Ff][Ee][Aa][Tt]\.?\s+|\s+[Ff][Tt]\.?\s+|\s+[Ff][Ee][Aa][Tt][Uu][Rr][Ii][Nn][Gg]\s+|\s+&\s*|\s+[Aa][Nn][Dd]\s+|\s+x\s+/g;

/**
 * Splits a raw artist string into parts and preserves the delimiters between them
 * so we can render each name as a clickable link while keeping the original format.
 *
 * When `artists` is provided (parsed track artists from the server), each rendered
 * name is matched to an artist id by normalized name and links to `/artist/:id`.
 * If no match is found, it falls back to `/search?q=...`.
 */
export function ArtistLinks({
  raw,
  artistId,
  artists,
  className = '',
  linkClassName = '',
}: {
  raw: string | null;
  artistId?: number | null;
  artists?: TrackArtist[];
  className?: string;
  linkClassName?: string;
}) {
  const navigate = useNavigate();
  const parts = useMemo(() => splitWithDelimiters(raw ?? 'Unknown Artist'), [raw]);
  const artistMap = useMemo(() => buildArtistMap(artists), [artists]);

  if (!raw) {
    return <span className={className}>Unknown Artist</span>;
  }

  // if there's only one part, link it directly to artistId if available
  if (parts.length === 1 && artistId) {
    return (
      <button
        onClick={(e) => {
          e.stopPropagation();
          navigate(`/artist/${artistId}`);
        }}
        className={`${className} ${linkClassName} hover:underline`}
      >
        {parts[0].text}
      </button>
    );
  }

  return (
    <span className={className}>
      {parts.map((part, i) => {
        if (part.isDelim) {
          return <span key={i}>{part.text}</span>;
        }
        const id = artistMap.get(normalize(part.text));
        return (
          <button
            key={i}
            onClick={(e) => {
              e.stopPropagation();
              e.preventDefault();
              if (id != null) {
                navigate(`/artist/${id}`);
              } else {
                navigate(`/search?q=${encodeURIComponent(part.text)}`);
              }
            }}
            className={`${linkClassName} hover:underline`}
          >
            {part.text}
          </button>
        );
      })}
    </span>
  );
}

function buildArtistMap(artists: TrackArtist[] | undefined): Map<string, number> {
  const map = new Map<string, number>();
  if (!artists) return map;
  for (const a of artists) {
    const key = normalize(a.name);
    if (!map.has(key)) map.set(key, a.id);
  }
  return map;
}

function normalize(name: string): string {
  return name
    .toLowerCase()
    .trim()
    .replace(/\s+/g, ' ');
}

interface SplitPart {
  text: string;
  isDelim: boolean;
}

function splitWithDelimiters(input: string): SplitPart[] {
  const result: SplitPart[] = [];
  let lastIndex = 0;
  DELIMS.lastIndex = 0;
  let match: RegExpExecArray | null;

  while ((match = DELIMS.exec(input)) !== null) {
    // text before the delimiter
    const before = input.slice(lastIndex, match.index);
    if (before.trim()) {
      result.push({ text: before.trim(), isDelim: false });
    }
    // the delimiter itself
    result.push({ text: match[0], isDelim: true });
    lastIndex = match.index + match[0].length;
  }

  // remaining text after last delimiter
  const after = input.slice(lastIndex);
  if (after.trim()) {
    result.push({ text: after.trim(), isDelim: false });
  }

  if (result.length === 0 && input.trim()) {
    result.push({ text: input.trim(), isDelim: false });
  }

  return result;
}
