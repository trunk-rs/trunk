import { test, expect } from './fixtures.js';

test('yew-tailwindcss release output works', async ({ page, logged }) => {
  const response = await page.goto('/');
  expect(response.ok()).toBe(true);
  await expect(page.locator('body')).toContainText('Trunk | Yew | Tailwind');
  await expect(page.locator('div.flex.flex-col.h-screen')).toHaveCSS('flex-direction', 'column');
  for (const image of await page.locator('img').all()) {
    await expect.poll(() => image.evaluate(i => i.complete && i.naturalWidth > 0)).toBe(true);
  }
});
