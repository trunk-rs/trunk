import { test, expect } from '../ci/fixtures.js';

test('yew-tls release output works', async ({ page, logged }) => {
  const response = await page.goto('/');
  expect(response.ok()).toBe(true);
  await expect(page.locator('body')).toContainText('Location Demo');
  await expect(page.locator('body')).toContainText('The current location is: http://127.0.0.1:8080/');
  await logged('Hello from JS FFI!');
});
