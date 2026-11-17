import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import brand from './brand/brand.json' with { type: 'json' };
import { headTags } from './vite/head.mjs';
import { serviceWorker } from './vite/service-worker.mjs';

// No inline script or style may reach the page (the Worker's CSP forbids both): assets are
// always emitted as files, and the module preload polyfill (an inline script) is off.
// A build needs SITE_BUILD_SHA (the commit being built): it versions the service worker's cache.
export default defineConfig(({ command }) => ({
  plugins: [
    react(),
    { name: 'synchronized-work-head', transformIndexHtml: () => headTags(brand) },
    ...(command === 'build' ? [serviceWorker()] : []),
  ],
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    assetsInlineLimit: 0,
    modulePreload: { polyfill: false },
    sourcemap: false,
    target: 'es2022',
  },
}));

