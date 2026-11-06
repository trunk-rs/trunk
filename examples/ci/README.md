# Example browser tests

Install dependencies with `npm ci`, then `npx playwright install --with-deps chromium`.
From this directory run `EXAMPLE=vanilla TRUNK_BIN=/absolute/path/to/trunk npm test`.
On Windows use an absolute Windows executable path. Tests build and serve the
selected example; ports 8080 and (for proxy) 9090 must be free.

CI caches npm downloads, Chromium and Trunk tool downloads. Failed tests retain
traces, screenshots and browser logs under test-results. Server output appears
in the job log. External browser resources are replaced with empty responses;
these tests cover local assets rather than CDN availability.
