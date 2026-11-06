import { test, expect } from './fixtures.js';

test('node-module release output works', async ({ page, logged }) => {
  const response = await page.goto('/');
  expect(response.ok()).toBe(true);
  await expect(page.locator('body')).toContainText('Primary');
  await expect(page.getByRole('button', { name: 'Primary' })).toHaveCSS('display', 'inline-flex');
});
