import type { Config } from 'tailwindcss';

const config: Config = {
  content: ['./index.html', './src/**/*.{ts,tsx}'],
  theme: {
    extend: {
      colors: {
        // LocalWave palette — derived from Spotify-inspired DESIGN.md
        base: {
          DEFAULT: '#121212',
          deep: '#0a0a0a',
        },
        surface: {
          DEFAULT: '#181818',
          mid: '#1f1f1f',
          card: '#252525',
          alt: '#272727',
        },
        brand: {
          DEFAULT: '#1ed760',
          dark: '#1db954',
          hover: '#1fdf64',
        },
        ink: {
          base: '#ffffff',
          muted: '#b3b3b3',
          soft: '#cbcbcb',
          faint: '#7c7c7c',
        },
        semantic: {
          neg: '#f3727f',
          warn: '#ffa42b',
          info: '#539df5',
        },
        edge: {
          DEFAULT: '#4d4d4d',
          light: '#7c7c7c',
        },
      },
      fontFamily: {
        sans: [
          'SpotifyMixUI',
          'CircularSp',
          'CircularSp-Arab',
          'Helvetica Neue',
          'Helvetica',
          'Arial',
          'sans-serif',
        ],
        title: [
          'SpotifyMixUITitle',
          'CircularSp',
          'Helvetica Neue',
          'Helvetica',
          'Arial',
          'sans-serif',
        ],
      },
      borderRadius: {
        pill: '9999px',
        'pill-lg': '500px',
      },
      boxShadow: {
        elev: 'rgba(0,0,0,0.3) 0px 8px 8px',
        dialog: 'rgba(0,0,0,0.5) 0px 8px 24px',
        insetedge: 'rgb(18,18,18) 0px 1px 0px, rgb(124,124,124) 0px 0px 0px 1px inset',
      },
      letterSpacing: {
        button: '1.4px',
      },
      fontSize: {
        micro: ['10px', 'normal'],
        badge: ['10.5px', '1.33'],
      },
    },
  },
  plugins: [],
};

export default config;
