// The service worker (emitted as /sw.js by vite/service-worker.mjs, which prefixes VERSION and
// PRECACHE). It makes the site installable and gives it an offline page; it never stands in
// for the node: the health check, the Citadel socket, sign-in and the well-known documents
// always go to the network, and nothing is cached at run time.
/* global VERSION, PRECACHE */

/** Paths that are never answered from a cache, matched as a path or a path prefix. */
const NEVER_CACHED = ['/healthz', '/citadel', '/auth/', '/.well-known/'];
const OFFLINE = '/offline';

function neverCached(pathname) {
  return NEVER_CACHED.some((p) => (p.endsWith('/') ? pathname.startsWith(p) || pathname === p.slice(0, -1) : pathname === p));
}

self.addEventListener('install', (event) => {
  event.waitUntil(
    caches
      .open(VERSION)
      .then((cache) => cache.addAll(PRECACHE))
      .then(() => self.skipWaiting()),
  );
});

self.addEventListener('activate', (event) => {
  event.waitUntil(
    caches
      .keys()
      .then((keys) => Promise.all(keys.filter((k) => k !== VERSION).map((k) => caches.delete(k))))
      .then(() => self.clients.claim()),
  );
});

self.addEventListener('fetch', (event) => {
  const request = event.request;
  const url = new URL(request.url);
  if (request.method !== 'GET' || url.origin !== self.location.origin || neverCached(url.pathname)) return;
  if (request.mode === 'navigate') {
    event.respondWith(fetch(request).catch(() => caches.match(OFFLINE, { cacheName: VERSION }).then((r) => r ?? Response.error())));
    return;
  }
  event.respondWith(caches.match(request, { cacheName: VERSION }).then((hit) => hit ?? fetch(request)));
});
