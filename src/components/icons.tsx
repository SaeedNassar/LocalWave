import type { SVGProps } from 'react';

const base = (props: SVGProps<SVGSVGElement>): SVGProps<SVGSVGElement> => ({
  width: 16,
  height: 16,
  viewBox: '0 0 24 24',
  fill: 'currentColor',
  xmlns: 'http://www.w3.org/2000/svg',
  ...props,
});

export const HomeIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base(p)} fill="none" stroke="currentColor" strokeWidth={2}>
    <path d="M3 12l9-9 9 9M5 10v10h5v-6h4v6h5V10" strokeLinecap="round" strokeLinejoin="round" />
  </svg>
);

export const SearchIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base(p)} fill="none" stroke="currentColor" strokeWidth={2}>
    <circle cx="11" cy="11" r="7" />
    <path d="M21 21l-4.3-4.3" strokeLinecap="round" />
  </svg>
);

export const LibraryIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base(p)} fill="none" stroke="currentColor" strokeWidth={2}>
    <path d="M3 5v14M8 5v14M13 5l4 14" strokeLinecap="round" strokeLinejoin="round" />
  </svg>
);

export const PlayIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base(p)}>
    <path d="M7 5v14l12-7z" />
  </svg>
);

export const PauseIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base(p)}>
    <path d="M6 4h4v16H6zM14 4h4v16h-4z" />
  </svg>
);

export const NextIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base(p)}>
    <path d="M5 5v14l9-7zM16 5h2v14h-2z" />
  </svg>
);

export const PrevIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base(p)}>
    <path d="M7 5h2v14H7zM9 12l9 7V5z" />
  </svg>
);

export const ShuffleIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base(p)} fill="none" stroke="currentColor" strokeWidth={2}>
    <path d="M16 4h4v4M4 20l16-16M4 4l5 5M15 15l5 5M16 20h4v-4" strokeLinecap="round" strokeLinejoin="round" />
  </svg>
);

export const RepeatIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base(p)} fill="none" stroke="currentColor" strokeWidth={2}>
    <path d="M17 1l4 4-4 4M3 11V9a4 4 0 014-4h14M7 23l-4-4 4-4M21 13v2a4 4 0 01-4 4H3" strokeLinecap="round" strokeLinejoin="round" />
  </svg>
);

export const RepeatOneIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base(p)} fill="none" stroke="currentColor" strokeWidth={2}>
    <path d="M17 1l4 4-4 4M3 11V9a4 4 0 014-4h14M7 23l-4-4 4-4M21 13v2a4 4 0 01-4 4H3" strokeLinecap="round" strokeLinejoin="round" />
    <text x="12" y="15" textAnchor="middle" fontSize="8" fill="currentColor" stroke="none" fontWeight="bold">1</text>
  </svg>
);

export const VolumeIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base(p)}>
    <path d="M3 9v6h4l5 5V4L7 9H3z" />
    <path d="M16 8a5 5 0 010 8" fill="none" stroke="currentColor" strokeWidth={2} strokeLinecap="round" />
    <path d="M19 5a9 9 0 010 14" fill="none" stroke="currentColor" strokeWidth={2} strokeLinecap="round" />
  </svg>
);

export const MuteIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base(p)}>
    <path d="M3 9v6h4l5 5V4L7 9H3z" />
    <path d="M22 9l-6 6M16 9l6 6" stroke="currentColor" strokeWidth={2} strokeLinecap="round" fill="none" />
  </svg>
);

export const HeartIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base(p)} fill="none" stroke="currentColor" strokeWidth={2}>
    <path d="M20.8 4.6a5.5 5.5 0 00-7.8 0L12 5.6l-1-1a5.5 5.5 0 10-7.8 7.8l8.8 8.8 8.8-8.8a5.5 5.5 0 000-7.8z" strokeLinejoin="round" />
  </svg>
);

export const HeartFilledIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base(p)}>
    <path d="M20.8 4.6a5.5 5.5 0 00-7.8 0L12 5.6l-1-1a5.5 5.5 0 10-7.8 7.8l8.8 8.8 8.8-8.8a5.5 5.5 0 000-7.8z" />
  </svg>
);

export const PlusIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base(p)} fill="none" stroke="currentColor" strokeWidth={2.5}>
    <path d="M12 5v14M5 12h14" strokeLinecap="round" />
  </svg>
);

export const QueueIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base(p)} fill="none" stroke="currentColor" strokeWidth={2}>
    <path d="M3 6h13M3 12h13M3 18h9M17 14v6M20 17l-3-3-3 3" strokeLinecap="round" strokeLinejoin="round" />
  </svg>
);

export const DotsIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base(p)}>
    <circle cx="5" cy="12" r="2" />
    <circle cx="12" cy="12" r="2" />
    <circle cx="19" cy="12" r="2" />
  </svg>
);

export const MusicNoteIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base(p)}>
    <path d="M12 3v10.55A4 4 0 1014 17V7h4V3h-6z" />
  </svg>
);

export const CloseIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base(p)} fill="none" stroke="currentColor" strokeWidth={2}>
    <path d="M6 6l12 12M18 6L6 18" strokeLinecap="round" />
  </svg>
);

export const ChevronDownIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base(p)} fill="none" stroke="currentColor" strokeWidth={2}>
    <path d="M6 9l6 6 6-6" strokeLinecap="round" strokeLinejoin="round" />
  </svg>
);
