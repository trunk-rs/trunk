import { test, expect } from './fixtures.js';

test('no-rust release output works', async ({ page, logged }) => {
  const response = await page.goto('/');
  expect(response.ok()).toBe(true);
  await expect(page.locator('body')).toContainText('Trunk without WASM');
  await expect(page.locator('h1')).toHaveCSS('text-align', 'center');
});
