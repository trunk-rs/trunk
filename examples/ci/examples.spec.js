import { test, expect } from '@playwright/test';
const example = process.env.EXAMPLE;
test('example works in Chromium', async ({ page }, info) => {
  const logs = [], errors = [];
  page.on('console', m => { logs.push(m.text()); if (m.type() === 'error') errors.push(m.text()); });
  page.on('pageerror', e => errors.push(String(e)));
  page.on('response', r => { if (new URL(r.url()).hostname === '127.0.0.1' && r.status() >= 400) errors.push(`${r.status()} ${r.url()}`); });
  page.on('requestfailed', r => { if (r.url().startsWith('http://127.0.0.1') || r.url().startsWith('https://127.0.0.1')) errors.push(r.url()); });
  await page.route(/^https?:\/\/(?!127\.0\.0\.1)/, route => route.fulfill({ status: 200, body: '' }));
  const logged = text => expect.poll(() => logs.join('\n'), { timeout: 30000 }).toContain(text);
  try {
    const response = await page.goto('/');
    expect(response.ok()).toBe(true);
    if (example.startsWith('webworker')) {
      await logged(example === 'webworker-gloo' ? '2 * 5 = 10' : '2 x 5 = 10');
      if (example === 'webworker-gloo') { await logged('3 * 3 = 9'); await logged('50 * 5 = 250'); }
    } else if (example === 'wasm_threads') {
      expect(await page.evaluate(() => crossOriginIsolated)).toBe(true);
      await logged('from the spawned thread'); await logged('Scope done x = 4, a.len() = 4');
    } else if (example === 'leptos') {
      await expect(page.getByText('Hello, world!', { exact: true })).toBeVisible();
    } else if (example === 'seed') {
      await page.getByPlaceholder('What needs to be done?').fill('Browser test');
      await page.getByPlaceholder('What needs to be done?').press('Enter');
      await page.locator('.todo-list .toggle').check();
      await expect(page.locator('.todo-list li')).toHaveClass('completed');
    } else {
      const text = example === 'no-rust' ? 'Trunk without WASM' : example === 'node-module' ? 'Primary' : example === 'leptos' ? 'Hello, world!' : example === 'yew-tls' ? 'Location Demo' : example === 'yew-tailwindcss' ? 'Trunk | Yew | Tailwind' : example === 'yew' ? 'Trunk | Yew | YBC' : example === 'cdylib' ? 'Hello, world from cdylib Rust!' : example === 'initializer' ? 'Hello, world from Vanilla Rust with an initializer!' : 'Hello, world from Vanilla Rust!';
      await expect(page.locator('body')).toContainText(text);
    }
    if (example === 'initializer') await logged('Loading... successful!');
    if (example === 'hooks') { await logged('I was generated at'); await logged('Feature A is'); }
    if (example === 'leptos') await logged('Hello World');
    if (['vanilla', 'target-path', 'cdylib', 'yew', 'yew-tls'].includes(example)) await logged('Hello from JS FFI!');
    if (example === 'vanilla') await logged('Hello from WASM!');
    if (['vanilla', 'target-path', 'cdylib'].includes(example)) await logged('Hello from JavaScript');
    if (['vanilla', 'target-path'].includes(example)) await logged('Hello from JavaScript Module');
    for (const image of await page.locator('img').all()) await expect.poll(() => image.evaluate(i => i.complete && i.naturalWidth > 0)).toBe(true);
    if (example === 'no-rust') await expect(page.locator('h1')).toHaveCSS('text-align', 'center');
    if (example === 'yew-tailwindcss') await expect(page.locator('nav')).toHaveCSS('height', '64px');
    if (example === 'node-module') await expect(page.getByRole('button', { name: 'Primary' })).toHaveCSS('display', 'inline-flex');
    if (example === 'target-path') {
      for (const asset of await page.locator('script[src], link[rel=stylesheet], img').all()) {
        const url = await asset.evaluate(el => el.src || el.href);
        expect(new URL(url).pathname).toMatch(/^\/static\//);
      }
    }
    if (example === 'yew-tls') await expect(page.locator('body')).toContainText('The current location is: http://127.0.0.1:8080/');
    expect(errors).toEqual([]);
  } finally {
    await info.attach('browser-log', { body: JSON.stringify({ logs, errors }), contentType: 'application/json' });
  }
});
