import { defineConfig } from '@playwright/test';
import { existsSync } from 'node:fs';
import path from 'node:path';
const example = process.env.EXAMPLE;
// An example is testable if it ships its own browser.spec.mjs; no central list.
if (!example || example !== path.basename(example) || example.startsWith('.')) throw new Error(`Invalid EXAMPLE: ${example}`);
if (!existsSync(path.resolve(import.meta.dirname, '..', '..', '..', 'examples', example, 'browser.spec.mjs'))) throw new Error(`EXAMPLE has no browser.spec.mjs: ${example}`);
const url = 'http://127.0.0.1:8080';
export default defineConfig({
  testDir: path.resolve(import.meta.dirname, '..', '..', '..', 'examples', example),
  testMatch: 'browser.spec.mjs',
  workers: 1, retries: 0, timeout: 60000,
  use: { browserName: 'chromium', baseURL: url, trace: 'retain-on-failure', screenshot: 'only-on-failure' },
  webServer: { command: 'node server.js', url, timeout: 30000, stdout: 'pipe', stderr: 'pipe' },
});
