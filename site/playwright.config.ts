import { defineConfig, devices } from '@playwright/test';

// The suite runs against `wrangler dev`: the real Worker (Rust, workerd) serving the built
// site through its own routes, so the CSP and /healthz under test are the shipped ones. Its
// node has no secrets, so /healthz is truly unprovisioned; the "ready" state is the one place
// a response is substituted (page.route), because no provisioned node exists to test against.
const PORT = 8799;

export default defineConfig({
  testDir: 'e2e',
  fullyParallel: true,
  forbidOnly: true,
  retries: 0,
  workers: 4,
  reporter: [['list']],
  use: { baseURL: `http://127.0.0.1:${PORT}`, trace: 'off' },
  projects: [
    { name: 'desktop', use: { ...devices['Desktop Chrome'], viewport: { width: 1280, height: 860 } } },
    { name: 'phone', use: { ...devices['Desktop Chrome'], viewport: { width: 390, height: 844 }, hasTouch: true } },
  ],
  webServer: {
    command: `npx wrangler dev --ip 127.0.0.1 --port ${PORT} --log-level warn --show-interactive-dev-session=false`,
    cwd: '..',
    url: `http://127.0.0.1:${PORT}/`,
    timeout: 600_000,
    reuseExistingServer: false,
    // No usage metrics to Cloudflare from a test run.
    env: { WRANGLER_SEND_METRICS: 'false' },
  },
});
