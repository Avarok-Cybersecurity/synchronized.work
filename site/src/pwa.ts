// Registers the service worker (/sw.js, emitted at build time by vite/service-worker.mjs) that
// makes the site installable and gives it an offline page. A module, never an inline script.
// A failed registration is logged, never thrown: the site works without it.
export function registerServiceWorker(): void {
  if (!('serviceWorker' in navigator)) return;
  navigator.serviceWorker.register('/sw.js', { scope: '/' }).catch((error: unknown) => {
    console.error('service worker registration failed', error);
  });
}
