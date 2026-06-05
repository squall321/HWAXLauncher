import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

// Tauri expects a fixed dev server on 5173 and must not clear the terminal,
// so the Rust build log stays visible alongside Vite's. See v2 plan §10.
export default defineConfig({
  plugins: [react()],
  // Prevent Vite from obscuring Rust/Tauri errors.
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    // Tauri dev reads files over http; HMR needs the same host.
    host: '127.0.0.1',
    watch: {
      // Don't watch the Rust side — the Tauri CLI handles that.
      ignored: ['**/src-tauri/**'],
    },
  },
  // Produce a static bundle the Tauri shell embeds; relative base keeps asset
  // URLs working from the tauri:// custom protocol.
  base: './',
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    target: 'esnext',
    sourcemap: false,
  },
});
