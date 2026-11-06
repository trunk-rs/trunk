import { test, expect } from '../ci/fixtures.js';

test('cdylib release output works', async ({ page, logged }) => {
  const response = await page.goto('/');
  expect(response.ok()).toBe(true);
  await expect(page.locator('body')).toContainText('Hello, world from cdylib Rust!');
  
  await logged('Hello from JS FFI!');
  await logged('Hello from JavaScript');
});
