import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

const host = process.env.TAURI_DEV_HOST;

// https://vitejs.dev/config/
export default defineConfig({
    plugins: [react()],

    // Tauri injects env vars; tell Vite to expose them.
    envPrefix: ['VITE_', 'TAURI_ENV_*'],

    // Tauri owns the console output for prettier dev UX.
    clearScreen: false,

    server: {
        port: 5173,
        strictPort: true,
        host: host ?? false,
        hmr: host
            ? {
                  protocol: 'ws',
                  host,
                  port: 1421,
              }
            : undefined,
        watch: {
            ignored: ['**/apps/desktop/**', '**/target/**'],
        },
    },

    build: {
        target: 'es2022',
        // Smaller bundles; Tauri ships the webview which already supports modern JS.
        minify: 'esbuild',
        sourcemap: false,
    },
});
