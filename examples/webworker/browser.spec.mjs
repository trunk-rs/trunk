import { test, expect } from '../ci/fixtures.js';

test('webworker release output works', async ({ page, logged }) => {
  const response = await page.goto('/');
  expect(response.ok()).toBe(true);
  await logged('2 x 5 = 10');
});
