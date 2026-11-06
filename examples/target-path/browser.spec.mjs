import { test, expect } from '../ci/fixtures.js';

test('target-path release output works', async ({ page, logged }) => {
  const response = await page.goto('/');
  expect(response.ok()).toBe(true);
  await expect(page.locator('body')).toContainText('Hello, world from Vanilla Rust!');
  
  await logged('Hello from JS FFI!');
  await logged('Hello from JavaScript');
  await logged('Hello from JavaScript Module');
  for (const image of await page.locator('img').all()) {
    await expect.poll(() => image.evaluate(i => i.complete && i.naturalWidth > 0)).toBe(true);
  }
  for (const asset of await page.locator('script[src], link[rel=stylesheet], img').all()) {
    const url = await asset.evaluate(el => el.src || el.href);
    expect(new URL(url).pathname).toMatch(/^\/static\//);
  }
});
