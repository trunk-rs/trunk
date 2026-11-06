import { test, expect } from './fixtures.js';

test('vanilla release output works', async ({ page, logged }) => {
  const response = await page.goto('/');
  expect(response.ok()).toBe(true);
  await expect(page.locator('body')).toContainText('Hello, world from Vanilla Rust!');
  
  await logged('Hello from JS FFI!');
  await logged('Hello from WASM!');
  await logged('Hello from JavaScript');
  await logged('Hello from JavaScript Module');
});
