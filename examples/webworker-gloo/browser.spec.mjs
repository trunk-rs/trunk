import { test, expect } from '../ci/fixtures.js';

test('webworker-gloo release output works', async ({ page, logged }) => {
  const response = await page.goto('/');
  expect(response.ok()).toBe(true);
  await logged('2 * 5 = 10');
  await logged('3 * 3 = 9');
  await logged('50 * 5 = 250');
});
