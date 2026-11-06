import { test, expect } from '../ci/fixtures.js';

test('wasm_threads release output works', async ({ page, logged }) => {
  const response = await page.goto('/');
  expect(response.ok()).toBe(true);
  expect(await page.evaluate(() => crossOriginIsolated)).toBe(true);
  await logged('from the spawned thread');
  await logged('Scope done x = 4, a.len() = 4');
});
