import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import path from 'node:path';

// Tauri expects a fixed port and clearScreen disabled during dev.
export default defineConfig({
  plugins: [react()],
  // Produce relative asset paths so the bundle works under tauri://localhost
  base: './',
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  // Prevent Tauri from watching the Rust src dir and vice versa.
  server: {
    port: 1420,
    strictPort: true,
    host: '127.0.0.1',
  },
  envPrefix: ['VITE_', 'TAURI_'],
  build: {
    // Tauri webview supports modern ES; smaller, faster bundles.
    target: 'es2021',
    minify: 'esbuild',
    sourcemap: false,
  },
});
