import { expect, test } from '@playwright/test';
import { icoEntries, ihdr } from '../scripts/brand/png.mjs';

// Every identity file, fetched from the real Worker: a 200 is not proof (the old SPA fallback
// answered every one of these paths 200 text/html), so each is checked for its type and its
// pixel size read from the bytes. The sizes are the platforms' requirements, not brand.json's.
const PNGS: [string, number, number][] = [
  ['/apple-touch-icon.png', 180, 180],
  ['/icon-192.png', 192, 192],
  ['/icon-512.png', 512, 512],
  ['/icon-maskable-512.png', 512, 512],
  ['/og-image.png', 1200, 630],
];
const ICO_TYPES = ['image/x-icon', 'image/vnd.microsoft.icon'];

test('every PNG is served as image/png at its required size', async ({ request }) => {
  for (const [path, width, height] of PNGS) {
    const r = await request.get(path);
    expect(r.status(), path).toBe(200);
    expect(r.headers()['content-type'], path).toBe('image/png');
    expect(ihdr(await r.body()), path).toMatchObject({ width, height });
  }
});

test('favicon.ico holds 16, 32 and 48 px images', async ({ request }) => {
  const r = await request.get('/favicon.ico');
  expect(r.status()).toBe(200);
  expect(ICO_TYPES).toContain(r.headers()['content-type']);
  const entries: { width: number; height: number; image: Uint8Array }[] = icoEntries(await r.body());
  expect(entries.map((e) => [e.width, e.height])).toEqual([[16, 16], [32, 32], [48, 48]]);
  for (const e of entries) expect(ihdr(e.image)).toMatchObject({ width: e.width, height: e.height });
});

test('the SVGs, manifest and robots.txt are served with their own types', async ({ request }) => {
  for (const path of ['/logo.svg', '/favicon.svg']) {
    const r = await request.get(path);
    expect(r.status(), path).toBe(200);
    expect(r.headers()['content-type'], path).toBe('image/svg+xml');
    expect(await r.text(), path).toMatch(/^<svg\b/);
  }
  const manifest = await request.get('/site.webmanifest');
  expect(manifest.status()).toBe(200);
  expect(manifest.headers()['content-type']).toBe('application/manifest+json');
  expect(await manifest.json()).toMatchObject({ start_url: '/', display: 'standalone' });
  const robots = await request.get('/robots.txt');
  expect(robots.status()).toBe(200);
  expect(robots.headers()['content-type']).toMatch(/^text\/plain/);
  expect(await robots.text()).toMatch(/^User-agent: \*\nAllow: \/\n/);
});

test('a missing file is 404; an app route still gets the shell', async ({ request }) => {
  for (const path of ['/no-such-image.png', '/apple-touch-icon-precomposed.png', '/assets/missing.js', '/.env']) {
    const r = await request.get(path);
    expect(r.status(), path).toBe(404);
    expect(r.headers()['content-type'] ?? '', path).not.toContain('text/html');
  }
  for (const path of ['/p/demo', '/auth/return', '/no-such-page']) {
    const r = await request.get(path);
    expect(r.status(), path).toBe(200);
    expect(r.headers()['content-type'], path).toContain('text/html');
    expect(await r.text(), path).toContain('<div id="root">');
  }
});

test('the head carries the Open Graph, Twitter, theme-color, manifest and canonical tags', async ({ page, request }) => {
  await page.goto('/');
  const meta = (attr: string, key: string) => page.locator(`head meta[${attr}="${key}"]`).getAttribute('content');
  const link = (rel: string) => page.locator(`head link[rel="${rel}"]`).getAttribute('href');
  for (const key of ['og:title', 'og:description', 'og:url', 'og:site_name', 'og:image:alt']) {
    expect(await meta('property', key), key).toBeTruthy();
  }
  expect(await meta('property', 'og:type')).toBe('website');
  expect(await meta('property', 'og:image:width')).toBe('1200');
  expect(await meta('property', 'og:image:height')).toBe('630');
  expect(await meta('name', 'twitter:card')).toBe('summary_large_image');
  for (const key of ['twitter:title', 'twitter:description', 'twitter:image']) expect(await meta('name', key), key).toBeTruthy();
  const themes = await page.locator('head meta[name="theme-color"]').evaluateAll((els) => els.map((e) => [e.getAttribute('media'), e.getAttribute('content')]));
  expect(themes.map(([media]) => media)).toEqual(['(prefers-color-scheme: light)', '(prefers-color-scheme: dark)']);
  expect(themes.every(([, color]) => /^#[0-9a-f]{6}$/.test(color ?? ''))).toBe(true);
  expect(await link('manifest')).toBe('/site.webmanifest');
  expect(await link('apple-touch-icon')).toBe('/apple-touch-icon.png');

  const canonical = new URL(String(await link('canonical')));
  const image = new URL(String(await meta('property', 'og:image')));
  expect(canonical.protocol).toBe('https:');
  expect(image.protocol, 'og:image is an absolute https URL').toBe('https:');
  expect(image.origin).toBe(canonical.origin);
  expect(await meta('property', 'og:url')).toBe(canonical.href);
  expect(await meta('name', 'twitter:image')).toBe(image.href);
  // The deployed origin is not reachable from a test, so the image's path is fetched here.
  const served = await request.get(image.pathname);
  expect(served.status()).toBe(200);
  expect(served.headers()['content-type']).toBe('image/png');
  expect(ihdr(await served.body())).toMatchObject({ width: 1200, height: 630 });
});
