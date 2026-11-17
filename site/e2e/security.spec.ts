import { expect, test } from '@playwright/test';
import { watchCsp } from './support';

// The directives that make the policy strict; the full value is crates/sw-worker/src/security.rs.
const STRICT = ["default-src 'none'", "script-src 'self' 'wasm-unsafe-eval'", "style-src 'self'", "connect-src 'self'", "worker-src 'self'", "manifest-src 'self'", "frame-ancestors 'none'", "base-uri 'none'"];

test('every page and asset carries the Worker\'s strict CSP', async ({ request }) => {
  const page = await request.get('/');
  const html = await page.text();
  const asset = html.match(/src="(\/assets\/[^"]+\.js)"/)?.[1];
  expect(asset, 'the shell names its script').toBeTruthy();
  for (const path of ['/', '/p/demo', '/auth/return', String(asset), '/favicon.svg', '/og-image.png', '/sw.js', '/site.webmanifest', '/healthz']) {
    const csp = (await request.get(path)).headers()['content-security-policy'] ?? '';
    for (const directive of STRICT) expect(csp, `${path}: ${directive}`).toContain(directive);
    expect(csp, path).not.toContain('unsafe-inline');
  }
  expect(page.headers()['x-content-type-options']).toBe('nosniff');
});

test('the landing and the demo run with no CSP violation and no third-party request', async ({ page, baseURL }) => {
  const violations = await watchCsp(page);
  const foreign: string[] = [];
  page.on('request', (r) => {
    if (new URL(r.url()).origin !== baseURL) foreign.push(r.url());
  });
  await page.goto('/');
  await expect(page.locator('.sign-in')).toHaveAttribute('data-health', 'unprovisioned');
  await page.getByRole('link', { name: 'Demo', exact: true }).click();
  await expect(page.locator('.react-flow__node')).toHaveCount(14);
  expect(await violations()).toEqual([]);
  expect(foreign).toEqual([]);
});

test('the issuer\'s return POST is redirected to a page that says sign-in cannot finish yet', async ({ page, request }) => {
  const post = await request.post('/auth/return', { form: { code: 'ZTBN-PG9A', state: 'ozHjXJPho4EgyGbICkCj-g' }, maxRedirects: 0 });
  expect(post.status()).toBe(303);
  expect(post.headers()['location']).toBe('/auth/return');
  await page.goto('/auth/return');
  await expect(page.getByRole('heading', { name: "Sign-in can't finish here yet" })).toBeVisible();
});
