import { expect, type Page } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';

/** /healthz as a provisioned node answers it: the only substituted response in the suite. */
export async function nodeReady(page: Page): Promise<void> {
  await page.route('**/healthz', (route) =>
    route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ v: 1, sha: 'abc1234', node: 'ready' }) }),
  );
}

/** CSP violations the page reports, collected from before its first script runs. */
export async function watchCsp(page: Page): Promise<() => Promise<string[]>> {
  await page.addInitScript(() => {
    const w = window as unknown as { __csp: string[] };
    w.__csp = [];
    document.addEventListener('securitypolicyviolation', (e) => w.__csp.push(`${e.violatedDirective} ${e.blockedURI}`));
  });
  return () => page.evaluate(() => (window as unknown as { __csp: string[] }).__csp);
}

/**
 * Nothing scrolls sideways and nothing is cut off at the viewport's edges. `scrollWidth` alone
 * is blind here: the body clips horizontal overflow, so every visible element's box is checked
 * too (the graph canvas and the background pan and scale by design and are skipped).
 */
export async function expectNoHorizontalScroll(page: Page): Promise<void> {
  const { scroll, client, offenders } = await page.evaluate(() => {
    const width = document.documentElement.clientWidth;
    const out: string[] = [];
    for (const el of document.querySelectorAll('body *')) {
      if (el.closest('.react-flow, .bg-canvas, .sr-only, .skip')) continue;
      const r = el.getBoundingClientRect();
      if (r.width === 0 || r.height === 0) continue;
      if (r.right > width + 1 || r.left < -1) out.push(`${el.tagName.toLowerCase()}.${el.className} ${Math.round(r.left)}..${Math.round(r.right)}`);
    }
    return { scroll: document.documentElement.scrollWidth, client: width, offenders: out.slice(0, 8) };
  });
  expect(scroll, 'the page scrolls sideways').toBeLessThanOrEqual(client);
  expect(offenders, 'elements cut off at the viewport edge').toEqual([]);
}

/** WCAG 2.1 AA, with motion settled first so contrast is read from final colours. */
export async function expectAccessible(page: Page): Promise<void> {
  await page.evaluate(() => Promise.all(document.getAnimations().map((a) => a.finished.catch(() => undefined))));
  const result = await new AxeBuilder({ page }).withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa']).analyze();
  expect(result.violations.map((v) => `${v.id}: ${v.nodes.map((n) => n.target.join(' ')).join(', ')}`)).toEqual([]);
}
