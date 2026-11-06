import { test, expect } from '../ci/fixtures.js';

test('yew release output works', async ({ page, logged }) => {
  const response = await page.goto('/');
  expect(response.ok()).toBe(true);
  await expect(page.locator('body')).toContainText('Trunk | Yew | YBC');
  
  await logged('Hello from JS FFI!');
  for (const image of await page.locator('img').all()) {
    await expect.poll(() => image.evaluate(i => i.complete && i.naturalWidth > 0)).toBe(true);
  }
});
