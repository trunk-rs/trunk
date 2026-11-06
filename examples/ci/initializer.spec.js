import { test, expect } from './fixtures.js';

test('initializer release output works', async ({ page, logged }) => {
  const response = await page.goto('/');
  expect(response.ok()).toBe(true);
  await expect(page.locator('body')).toContainText('Hello, world from Vanilla Rust with an initializer!');
  await logged('Loading... successful!');
});
