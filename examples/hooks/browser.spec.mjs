import { test, expect } from '../ci/fixtures.js';

test('hooks release output works', async ({ page, logged }) => {
  const response = await page.goto('/');
  expect(response.ok()).toBe(true);
  await expect(page.locator('body')).toContainText('Hello, world from Vanilla Rust!');
  await logged('I was generated at');
  await logged('Feature A is');
});
