/// <reference types="vitest/config" />
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import { VitePWA } from 'vite-plugin-pwa'
// Added (TMAIL-259): bundle visualizer — writes dist/stats.html on every build
// so we have a permanent record of chunk shape after each Vite change.
import { visualizer } from 'rollup-plugin-visualizer'

export default defineConfig({
  // Added (TMAIL-259): manual vendor chunking. Splits the three largest
  // third-party libraries out of the main bundle so each one is a separate
  // cacheable file. Combined with React.lazy() on the Settings managers
  // (AppShell.tsx) and the auxiliary routes (App.tsx), this brings the
  // initial entry under the 300 kB gzip threshold from TMAIL-241.
  build: {
    rolldownOptions: {
      output: {
        // Vite 8 ships Rolldown, which takes manualChunks as a function rather
        // than the Rollup object map. Returning a chunk name pulls the matched
        // module into that named chunk; returning undefined lets the bundler
        // decide. Vendor names match dist/assets/<name>-<hash>.js so the
        // stats.html report stays human-readable.
        manualChunks: (id: string) => {
          if (id.includes('node_modules/')) {
            if (
              id.includes('/react/') ||
              id.includes('/react-dom/') ||
              id.includes('/react-router/') ||
              id.includes('/react-router-dom/') ||
              id.includes('/scheduler/')
            ) {
              return 'react-vendor'
            }
            if (id.includes('/@tanstack/')) {
              return 'query-vendor'
            }
            if (id.includes('/@tiptap/') || id.includes('/prosemirror') || id.includes('/dompurify')) {
              return 'editor-vendor'
            }
            if (id.includes('/@fullcalendar/')) {
              return 'calendar-vendor'
            }
          }
          return undefined
        },
      },
    },
  },
  plugins: [
    react(),
    // Added (TMAIL-259): emit dist/stats.html with gzip + brotli sizes so the
    // bundle-size assessment can be re-run by re-reading the artifact.
    visualizer({
      filename: 'dist/stats.html',
      template: 'treemap',
      gzipSize: true,
      brotliSize: true,
    }),
    VitePWA({
      registerType: 'autoUpdate',
      includeAssets: ['favicon.svg'],
      manifest: {
        name: 'TASMail',
        short_name: 'TASMail',
        description: 'Self-hosted email service by Tech at Scale',
        theme_color: '#2563eb',
        background_color: '#ffffff',
        display: 'standalone',
        scope: '/',
        start_url: '/',
        icons: [
          { src: '/icon-192.png', sizes: '192x192', type: 'image/png' },
          { src: '/icon-512.png', sizes: '512x512', type: 'image/png' },
          { src: '/icon-512.png', sizes: '512x512', type: 'image/png', purpose: 'maskable' },
        ],
      },
      workbox: {
        globPatterns: ['**/*.{js,css,html,ico,png,svg,woff2}'],
        // Added: drop previous-revision precaches on SW upgrade so Cache Storage
        // doesn't accumulate one full precache per deploy (TMAIL-261 finding 8).
        cleanupOutdatedCaches: true,
        // NOTE: runtimeCaching is evaluated top-down. NetworkOnly first to ensure
        // privacy/correctness-sensitive routes never sit in any cache, then the
        // long-lived StaleWhileRevalidate for branding, then the catch-all
        // NetworkFirst for the rest of /api/* GETs. Full per-route plan is in
        // docs/assessments/frontend-pwa-offline-2026-05.md (TMAIL-261 finding 2);
        // the split below is the conservative subset that only *removes* caching
        // from routes that should never have been cached.
        runtimeCaching: [
          {
            // Privacy + per-query garbage + time-sensitive availability: never cache.
            urlPattern: /^https?:\/\/[^/]+\/api\/(auth|search|calendar\/free-busy)\b/,
            handler: 'NetworkOnly',
          },
          {
            // Branding rarely changes and every render reads it — SWR with 24h TTL.
            urlPattern: /^https?:\/\/[^/]+\/api\/branding\b/,
            method: 'GET',
            handler: 'StaleWhileRevalidate',
            options: {
              cacheName: 'branding-cache',
              expiration: { maxEntries: 4, maxAgeSeconds: 86400 },
            },
          },
          {
            // Catch-all for the rest of /api/* GETs. Explicit method:'GET' keeps
            // mutations off the SW cache path regardless of Workbox defaults.
            urlPattern: /^https?:\/\/[^/]+\/api\//,
            method: 'GET',
            handler: 'NetworkFirst',
            options: {
              cacheName: 'api-cache',
              expiration: { maxEntries: 100, maxAgeSeconds: 300 },
              networkTimeoutSeconds: 5,
            },
          },
        ],
      },
    }),
  ],
  test: {
    globals: true,
    environment: 'jsdom',
    setupFiles: ['./src/test/setup.ts'],
    // Added: Exclude Playwright E2E specs from Vitest runner
    exclude: ['e2e/**', 'node_modules/**'],
  },
  server: {
    // Changed: Non-default port (5173 occupied by Alleina dev server)
    port: Number(process.env.TASMAIL_VITE_PORT ?? 5273),
    host: '127.0.0.1',
    // Added: Allow Apache reverse proxy from mail.techatscale.io to forward host header
    allowedHosts: ['mail.techatscale.io', 'localhost', '127.0.0.1'],
    // Added: Required so HMR works through the SSH tunnel + Apache reverse proxy
    hmr: {
      host: 'mail.techatscale.io',
      protocol: 'wss',
      clientPort: 443,
    },
    proxy: {
      '/api': {
        // Changed: Backend default port shifted from 3000 -> 3300 to avoid collisions
        target: `http://127.0.0.1:${process.env.TASMAIL_BACKEND_PORT ?? 3300}`,
        changeOrigin: true,
      },
      '/ws': {
        target: `ws://127.0.0.1:${process.env.TASMAIL_BACKEND_PORT ?? 3300}`,
        ws: true,
      },
      // Added (TMAIL-421): /classic/* is the no-JS Classic UI surface owned
      // by the Rust backend (handlers::classic::router). Without this entry
      // every /classic/login | /classic/folders/... request was being caught
      // by Vite's SPA fallback and answered with index.html, breaking every
      // classic-* E2E spec when the test base URL ran through the dev
      // tunnel / Apache → Vite stack rather than directly against :3300.
      '/classic': {
        target: `http://127.0.0.1:${process.env.TASMAIL_BACKEND_PORT ?? 3300}`,
        changeOrigin: true,
      },
    },
  },
})
