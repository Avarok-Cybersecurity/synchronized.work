import { expect, test } from '@playwright/test';
import { expectAccessible, expectNoHorizontalScroll, nodeReady } from './support';

const signInButton = { name: 'Sign in with Citadel' } as const;

test('the real, unprovisioned /healthz shows that sign-in opens soon', async ({ page, request }) => {
  const health = await request.get('/healthz');
  expect(health.status()).toBe(503);
  expect(await health.json()).toMatchObject({ v: 1, node: 'unprovisioned' });
  await page.goto('/');
  const box = page.locator('.sign-in');
  await expect(box).toHaveAttribute('data-health', 'unprovisioned');
  await expect(box.getByRole('status')).toContainText('Sign-in opens soon.');
  await expect(page.getByRole('button', signInButton)).toHaveCount(0);
});

test('a healthy /healthz offers sign-in, and the link carries only the four parameters', async ({ page }) => {
  await nodeReady(page);
  await page.route('https://work.avarok.net/**', (route) => route.fulfill({ status: 200, contentType: 'text/plain', body: 'issuer' }));
  await page.goto('/');
  const button = page.getByRole('button', signInButton);
  await expect(button).toBeVisible();
  await button.click();
  await page.waitForURL('https://work.avarok.net/**');
  const url = new URL(page.url());
  expect(url.pathname).toBe('/sign-in');
  expect([...url.searchParams.keys()]).toEqual(['client', 'challenge', 'state', 'return']);
  expect(url.searchParams.get('client')).toBe('synchronized.work');
  expect(url.searchParams.get('return')).toBe('https://synchronized.work/auth/return');
  expect(url.searchParams.get('challenge')).toMatch(/^[A-Za-z0-9_-]{43}$/);
});

test('a /healthz that is not the contract is never shown as open', async ({ page }) => {
  await page.route('**/healthz', (route) => route.fulfill({ status: 200, contentType: 'application/json', body: '{"node":"ready"}' }));
  await page.goto('/');
  await expect(page.locator('.sign-in')).toHaveAttribute('data-health', 'unreachable');
  await expect(page.getByRole('button', signInButton)).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Check again' })).toBeVisible();
});

test('the landing explains the tree, the tiers, the MCP and post-quantum security', async ({ page }) => {
  await page.goto('/');
  for (const heading of ['One live tree, many people', 'Three tiers', 'The MCP', 'Post-quantum, end to end']) {
    await expect(page.getByRole('heading', { name: heading })).toBeVisible();
  }
  await expect(page.getByText('claude mcp add synchronized-work -- sw-mcp serve')).toBeVisible();
});

for (const scheme of ['light', 'dark'] as const) {
  test(`the landing passes WCAG AA in the ${scheme} theme without sideways scroll`, async ({ page }) => {
    await page.emulateMedia({ colorScheme: scheme, reducedMotion: 'reduce' });
    await page.goto('/');
    await expect(page.locator('.sign-in')).toHaveAttribute('data-health', 'unprovisioned');
    await expectNoHorizontalScroll(page);
    await expectAccessible(page);
  });
}

test('the theme toggle switches between light and dark', async ({ page }) => {
  await page.emulateMedia({ colorScheme: 'light' });
  await page.goto('/');
  const bg = () => page.evaluate(() => getComputedStyle(document.body).backgroundColor);
  const light = await bg();
  await page.getByRole('button', { name: 'Dark theme' }).click();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  expect(await bg()).not.toBe(light);
});

test('reduced motion draws one still frame; otherwise the background animates', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.goto('/');
  await expect(page.locator('.bg-canvas')).toHaveAttribute('data-motion', 'still');
  await page.emulateMedia({ reducedMotion: 'no-preference' });
  await page.goto('/');
  await expect(page.locator('.bg-canvas')).toHaveAttribute('data-motion', 'animated');
});

test('the keyboard reaches the skip link, then the navigation, with a visible focus ring', async ({ page }) => {
  await page.goto('/');
  // The skip link is rendered by the app, not the shell: goto resolves on `load`, before React has
  // mounted it. A Tab pressed into the empty shell lands nowhere, and the link, once mounted, reads
  // `inactive` (run 44959413916). Tab through the DOM the test is about, not the one before it.
  const skip = page.getByRole('link', { name: 'Skip to content' });
  await skip.waitFor({ state: 'attached' });
  await page.keyboard.press('Tab');
  await expect(skip).toBeFocused();
  await page.keyboard.press('Tab');
  const outline = await page.evaluate(() => getComputedStyle(document.activeElement as Element).outlineStyle);
  expect(outline).not.toBe('none');
});
