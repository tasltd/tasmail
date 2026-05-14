import { defineConfig } from 'vite'
import path from 'path'
import tailwindcss from '@tailwindcss/vite'
import react from '@vitejs/plugin-react'

// TMAIL-215: alt-UI is served from /modern/ off the same Vite host as the
// production SPA (frontend/public/modern/ is its build output). Setting base
// here makes asset URLs absolute under /modern/, so the static files served
// by the parent Vite (or by Apache in production) resolve correctly.
//
// In dev (npm run dev inside themes/shadcn-prototype), proxy /api to the
// live backend on :3300 so the alt-UI can be developed against the same
// data the production SPA uses.
export default defineConfig({
  base: '/modern/',
  server: {
    proxy: {
      '/api': {
        target: 'http://127.0.0.1:3300',
        changeOrigin: true,
      },
      '/ws': {
        target: 'ws://127.0.0.1:3300',
        ws: true,
      },
    },
  },
  plugins: [
    // The React and Tailwind plugins are both required for Make, even if
    // Tailwind is not being actively used – do not remove them
    react(),
    tailwindcss(),
  ],
  resolve: {
    alias: {
      // Alias @ to the src directory
      '@': path.resolve(__dirname, './src'),
    },
  },

  // File types to support raw imports. Never add .css, .tsx, or .ts files to this.
  assetsInclude: ['**/*.svg', '**/*.csv'],
})
