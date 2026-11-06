import { test as base, expect } from '@playwright/test';
export { expect };
export const test = base.extend({
  logged: async ({ page }, use, info) => {
    const logs = [], errors = [];
    page.on('console', m => { logs.push(m.text()); if (m.type() === 'error') errors.push(m.text()); });
    page.on('pageerror', e => errors.push(String(e)));
    page.on('response', r => { if (new URL(r.url()).hostname === '127.0.0.1' && r.status() >= 400) errors.push(`${r.status()} ${r.url()}`); });
    page.on('requestfailed', r => { if (r.url().startsWith('http://127.0.0.1') || r.url().startsWith('https://127.0.0.1')) errors.push(r.url()); });
    await page.route(/^https?:\/\/(?!127\.0\.0\.1)/, route => route.fulfill({ status: 200, body: '' }));
    try {
      await use(text => expect.poll(() => logs.join('\n'), { timeout: 30000 }).toContain(text));
      expect(errors).toEqual([]);
    } finally {
      await info.attach('browser-log', { body: JSON.stringify({ logs, errors }), contentType: 'application/json' });
    }
  },
});
