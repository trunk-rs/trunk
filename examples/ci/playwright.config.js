import { defineConfig } from '@playwright/test';
import path from 'node:path';
const example = process.env.EXAMPLE;
const examples = 'cargo-manifest cdylib hooks initializer leptos no-rust node-module proxy seed target-path vanilla webworker webworker-gloo webworker-module wasm_threads yew yew-tailwindcss yew-tls yaml-config'.split(' ');
if (!examples.includes(example)) throw new Error(`Unknown EXAMPLE: ${example}`);
const binary = process.env.TRUNK_BIN;
if (!binary || !path.isAbsolute(binary)) throw new Error('TRUNK_BIN must be absolute');
const url = `${example === 'yew-tls' ? 'https' : 'http'}://127.0.0.1:8080`;
export default defineConfig({
  workers: 1, retries: 0, timeout: 60000,
  use: { browserName: 'chromium', baseURL: url, ignoreHTTPSErrors: example === 'yew-tls', trace: 'retain-on-failure', screenshot: 'only-on-failure' },
  webServer: { command: `"${binary}" serve --address 127.0.0.1 --port 8080 --no-autoreload`, cwd: path.resolve(import.meta.dirname, '..', example), url, ignoreHTTPSErrors: example === 'yew-tls', timeout: 900000, env: { TRUNK_REQUIRED_VERSION: '*' }, stdout: 'pipe', stderr: 'pipe' },
});
