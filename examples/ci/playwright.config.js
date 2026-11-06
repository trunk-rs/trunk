import { defineConfig } from '@playwright/test';
import path from 'node:path';
const example = process.env.EXAMPLE;
const examples = 'cargo-manifest cdylib hooks initializer leptos no-rust node-module seed target-path vanilla webworker webworker-gloo webworker-module wasm_threads yew yew-tailwindcss yew-tls yaml-config'.split(' ');
if (!examples.includes(example)) throw new Error(`Unknown EXAMPLE: ${example}`);
const url = 'http://127.0.0.1:8080';
export default defineConfig({
  testDir: path.resolve(import.meta.dirname, '..', example),
  testMatch: 'browser.spec.mjs',
  workers: 1, retries: 0, timeout: 60000,
  use: { browserName: 'chromium', baseURL: url, trace: 'retain-on-failure', screenshot: 'only-on-failure' },
  webServer: { command: 'node server.js', url, timeout: 30000, stdout: 'pipe', stderr: 'pipe' },
});
