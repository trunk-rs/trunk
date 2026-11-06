import { test, expect } from '../ci/fixtures.js';

test('leptos release output works', async ({ page, logged }) => {
  const response = await page.goto('/');
  expect(response.ok()).toBe(true);
  await expect(page.getByText('Hello, world!', { exact: true })).toBeVisible();
  await logged('Hello World');
});
