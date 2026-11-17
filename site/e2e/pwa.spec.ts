import { expect, test, type Page } from '@playwright/test';
import { ihdr } from '../scripts/brand/png.mjs';

// The site as an installable PWA, in Chromium against the real Worker: Chromium's own reading
// of the manifest and its install criteria (over CDP), the service worker controlling the page,
// the offline fallback, and the paths the service worker must never answer.

/** Loads the landing and waits until the service worker controls it (it claims its clients). */
async function controlled(page: Page): Promise<void> {
  await page.goto('/');
  await page.waitForFunction(() => navigator.serviceWorker.controller !== null, undefined, { timeout: 15_000 });
}

/** Every path in every cache the site holds. */
function cachedPaths(page: Page): Promise<string[]> {
  return page.evaluate(async () => {
    const out: string[] = [];
    for (const key of await caches.keys()) {
      for (const r of await (await caches.open(key)).keys()) out.push(new URL(r.url).pathname);
    }
    return out;
  });
}

test('the manifest parses, its icons load, and Chromium finds the site installable', async ({ page, request, baseURL }) => {
  await controlled(page);
  const cdp = await page.context().newCDPSession(page);
  const manifest = await cdp.send('Page.getAppManifest');
  expect(manifest.errors).toEqual([]);
  expect(manifest.url).toBe(`${baseURL}/site.webmanifest`);
  expect(manifest.data, 'Chromium read a manifest').toBeTruthy();
  const parsed = JSON.parse(String(manifest.data)) as Record<string, unknown> & { icons: { src: string; sizes: string; purpose: string }[] };
  expect(parsed).toMatchObject({ id: '/', start_url: '/', scope: '/', display: 'standalone' });
  for (const key of ['name', 'short_name', 'theme_color', 'background_color']) expect(parsed[key], key).toBeTruthy();
  expect(parsed.icons.map((i) => `${i.sizes} ${i.purpose}`)).toEqual(['192x192 any', '512x512 any', '512x512 maskable']);
  for (const icon of parsed.icons) {
    const r = await request.get(icon.src);
    expect(r.status(), icon.src).toBe(200);
    expect(r.headers()['content-type'], icon.src).toBe('image/png');
    const side = Number(icon.sizes.split('x')[0]);
    expect(ihdr(await r.body()), icon.src).toMatchObject({ width: side, height: side });
  }
  const { installabilityErrors } = await cdp.send('Page.getInstallabilityErrors');
  expect(installabilityErrors).toEqual([]);

  const registration = await page.evaluate(async () => {
    const r = await navigator.serviceWorker.getRegistration('/');
    return { scope: r?.scope, state: r?.active?.state, script: r?.active?.scriptURL, controller: navigator.serviceWorker.controller?.scriptURL };
  });
  expect(registration).toEqual({ scope: `${baseURL}/`, state: 'activated', script: `${baseURL}/sw.js`, controller: `${baseURL}/sw.js` });
  const sw = await request.get('/sw.js');
  expect(sw.headers()['cache-control']).toBe('no-cache');
  expect(sw.headers()['content-type']).toMatch(/javascript/);
});

test('offline, a navigation gets the offline page from the cache', async ({ page, context }) => {
  await controlled(page);
  await context.setOffline(true);
  await page.goto('/p/demo');
  await expect(page.getByRole('heading', { name: "You're offline" })).toBeVisible();
  await expect(page.getByRole('link', { name: 'Try again' })).toHaveAttribute('href', '/');
  await context.setOffline(false);
});

test('/healthz, /citadel, /auth/* and /.well-known/* never come from the service worker', async ({ page, context }) => {
  await controlled(page);
  const [health] = await Promise.all([
    page.waitForResponse((r) => new URL(r.url()).pathname === '/healthz'),
    page.reload(),
  ]);
  expect(health.fromServiceWorker(), '/healthz was answered by the service worker').toBe(false);
  const cached = await cachedPaths(page);
  expect(cached, 'the precache is not empty, so the next check means something').toContain('/offline');
  for (const path of ['/healthz', '/citadel', '/auth/return', '/.well-known/citadel']) expect(cached).not.toContain(path);

  await context.setOffline(true);
  const offline = await page.evaluate(() => fetch('/healthz').then((r) => `served ${r.status}`, () => 'network error'));
  expect(offline, 'offline, /healthz must fail rather than be answered from a cache').toBe('network error');
  await context.setOffline(false);
});
