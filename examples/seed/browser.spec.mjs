import { test, expect } from '../ci/fixtures.js';

test('seed release output works', async ({ page, logged }) => {
  const response = await page.goto('/');
  expect(response.ok()).toBe(true);
  await page.getByPlaceholder('What needs to be done?').fill('Browser test');
  await page.getByPlaceholder('What needs to be done?').press('Enter');
  await page.locator('.todo-list .toggle').check();
  await expect(page.locator('.todo-list li')).toHaveClass('completed');
});
