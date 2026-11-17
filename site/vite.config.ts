import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

// No inline script or style may reach the page (the Worker's CSP forbids both): assets are
// always emitted as files, and the module preload polyfill (an inline script) is off.
export default defineConfig({
  plugins: [react()],
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    assetsInlineLimit: 0,
    modulePreload: { polyfill: false },
    sourcemap: false,
    target: 'es2022',
  },
});
