import { test, expect } from '@playwright/test';
import http from 'node:http';
import { WebSocketServer } from 'ws';
const example = process.env.EXAMPLE;
test('example works in Chromium', async ({ page }, info) => {
  const logs = [], errors = [];
  page.on('console', m => { logs.push(m.text()); if (m.type() === 'error') errors.push(m.text()); });
  page.on('pageerror', e => errors.push(String(e)));
  page.on('response', r => { if (new URL(r.url()).hostname === '127.0.0.1' && r.status() >= 400) errors.push(`${r.status()} ${r.url()}`); });
  page.on('requestfailed', r => { if (r.url().startsWith('http://127.0.0.1') || r.url().startsWith('https://127.0.0.1')) errors.push(r.url()); });
  await page.route(/^https?:\/\/(?!127\.0\.0\.1)/, route => route.fulfill({ status: 200, body: '' }));
  let backend, sockets;
  if (example === 'proxy') {
    backend = http.createServer((req, res) => { let body = ''; req.on('data', c => body += c); req.on('end', () => { res.setHeader('content-type', 'application/json'); res.end(JSON.stringify({ url: req.url, headers: req.headers, body })); }); });
    sockets = new WebSocketServer({ server: backend });
    sockets.on('connection', ws => ws.on('message', data => ws.send(data.toString())));
    await new Promise(resolve => backend.listen(9090, '127.0.0.1', resolve));
  }
  const logged = text => expect.poll(() => logs.join('\n')).toContain(text);
  try {
    const response = await page.goto('/');
    if (example === 'yaml-config') expect(response.headers().foo).toBe('bar');
    if (example.startsWith('webworker')) {
      await logged(example === 'webworker-gloo' ? '2 * 5 = 10' : '2 x 5 = 10');
      if (example === 'webworker-gloo') { await logged('3 * 3 = 9'); await logged('50 * 5 = 250'); }
    } else if (example === 'wasm_threads') {
      expect(await page.evaluate(() => crossOriginIsolated)).toBe(true);
      await logged('from the spawned thread'); await logged('Scope done x = 4, a.len() = 4');
    } else if (example === 'seed') {
      await page.getByPlaceholder('What needs to be done?').fill('Browser test');
      await page.getByPlaceholder('What needs to be done?').press('Enter');
      await page.locator('.todo-list .toggle').check();
      await page.getByRole('link', { name: 'Active', exact: true }).click();
      await expect(page.locator('.todo-list li')).toHaveCount(0);
      await page.getByRole('link', { name: 'Completed', exact: true }).click();
      await page.reload(); await expect(page.locator('.todo-list li')).toHaveCount(1);
      await page.getByRole('button', { name: /Clear completed/ }).click();
      await expect(page.locator('.todo-list li')).toHaveCount(0);
    } else {
      const text = example === 'no-rust' ? 'Trunk without WASM' : example === 'node-module' ? 'Primary' : example === 'proxy' ? 'Trunk Proxy Demo App' : example === 'leptos' ? 'Hello, world!' : example === 'yew-tls' ? 'Location Demo' : example === 'yew-tailwindcss' ? 'Trunk | Yew | Tailwind' : example === 'yew' ? 'Trunk | Yew | YBC' : example === 'cdylib' ? 'Hello, world from cdylib Rust!' : example === 'initializer' ? 'Hello, world from Vanilla Rust with an initializer!' : 'Hello, world from Vanilla Rust!';
      await expect(page.locator('body')).toContainText(text);
    }
    if (example === 'initializer') await logged('Loading... successful!');
    if (example === 'hooks') await logged('I was generated at');
    if (example === 'leptos') await logged('Hello World');
    if (['vanilla', 'target-path', 'cdylib', 'yew', 'yew-tls'].includes(example)) await logged('Hello from JS FFI!');
    if (example === 'vanilla') await logged('Hello from WASM!');
    if (['vanilla', 'target-path', 'cdylib'].includes(example)) await logged('Hello from JavaScript');
    if (['vanilla', 'target-path'].includes(example)) await logged('Hello from JavaScript Module');
    for (const image of await page.locator('img').all()) await expect.poll(() => image.evaluate(i => i.complete && i.naturalWidth > 0)).toBe(true);
    if (example === 'proxy') {
      for (const [url, expected] of [['/api/v1/resource?q=1', '/resource?q=1'], ['/api/v2/resource?q=1', '/api/v2/resource?q=1']]) {
        const result = await page.evaluate(async url => (await fetch(url, { method: 'POST', body: 'hello' })).json(), url);
        expect(result.url).toBe(expected); expect(result.body).toBe('hello');
        if (url.includes('v1')) expect(result.headers['x-api-key']).toBe('some-special-key');
      }
      for (const route of ['/api/ws', '/api/websocket']) expect(await page.evaluate(route => new Promise((resolve, reject) => { const ws = new WebSocket(`ws://${location.host}${route}`); ws.onopen = () => ws.send('hello'); ws.onmessage = e => { resolve(e.data); ws.close(); }; ws.onerror = reject; }), route)).toBe('hello');
    }
    expect(errors).toEqual([]);
  } finally {
    await info.attach('browser-log', { body: JSON.stringify({ logs, errors }), contentType: 'application/json' });
    sockets?.clients.forEach(ws => ws.terminate()); sockets?.close();
    if (backend) await new Promise(resolve => backend.close(resolve));
  }
});
