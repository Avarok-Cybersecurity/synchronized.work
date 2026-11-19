import { expect, test } from '@playwright/test';
import { expectAccessible, expectNoHorizontalScroll } from './support';

test('the demo project is labelled as a demo and draws the whole fixture tree', async ({ page }) => {
  await page.goto('/p/demo');
  await expect(page.getByRole('note')).toContainText('Demo project.');
  await expect(page.locator('.react-flow__node')).toHaveCount(14);
  await expect(page.locator('.react-flow__edge')).toHaveCount(13);
  await expect(page.getByRole('treeitem', { name: /Agent: agent-7/ })).toBeVisible();
});

test('the outline is a keyboard tree that shares its selection with the graph', async ({ page }) => {
  await page.goto('/p/demo');
  const first = page.getByRole('treeitem').first();
  await first.focus();
  await page.keyboard.press('ArrowDown');
  const task = page.getByRole('treeitem', { name: /^Task: Ship the synchronized\.work site/ });
  await expect(task).toBeFocused();
  await expect(task).toHaveAttribute('aria-selected', 'true');
  await expect(page.locator('.detail')).toContainText('Ship the synchronized.work site');
  await expect(page.locator('.react-flow__node.selected')).toContainText('Ship the synchronized.work site');
  await page.keyboard.press('ArrowLeft');
  await expect(task).toHaveAttribute('aria-expanded', 'false');
});

for (const scheme of ['light', 'dark'] as const) {
  test(`the demo passes WCAG AA in the ${scheme} theme without sideways scroll`, async ({ page }) => {
    await page.emulateMedia({ colorScheme: scheme, reducedMotion: 'reduce' });
    await page.goto('/p/demo');
    await expect(page.locator('.react-flow__node')).toHaveCount(14);
    await expectNoHorizontalScroll(page);
    await expectAccessible(page);
  });
}
