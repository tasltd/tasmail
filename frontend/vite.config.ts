/// <reference types="vitest/config" />
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import { VitePWA } from 'vite-plugin-pwa'

export default defineConfig({
  plugins: [
    react(),
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
        runtimeCaching: [
          {
            urlPattern: /^https?:\/\/.*\/api\//,
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
    },
  },
})
