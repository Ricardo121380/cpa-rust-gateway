// Production-embed audit. Input is supplied over stdin; secrets never enter artifacts.
import { chromium } from '@playwright/test';
import { mkdir, writeFile } from 'node:fs/promises';
let input = '';
for await (const chunk of process.stdin) input += chunk;
const { base, key, csrf, output } = JSON.parse(input);
const routes = ['/', '/monitoring', '/usage', '/billing', '/accounts', '/upstreams', '/catalog', '/models', '/access', '/runtime', '/egress', '/versions', '/audit', '/settings'];
await mkdir(output, { recursive: true });
const browser = await chromium.launch();
const evidence = { browser: browser.version(), pages: [], errors: [] };
try {
  for (const [width, height] of [[1440, 900], [1280, 720], [390, 844]]) {
    for (const theme of ['light', 'dark']) {
      const context = await browser.newContext({ viewport: { width, height }, colorScheme: theme, reducedMotion: 'reduce' });
      const page = await context.newPage();
      page.on('pageerror', error => evidence.errors.push({ width, theme, message: error.message }));
      await page.goto(`${base}/admin-ui/#/unlock`);
      await page.getByRole('button', { name: '解锁', exact: true }).waitFor();
      await page.screenshot({ path: `${output}/${width}-${theme}-unlock.png` });
      await page.getByLabel('Management Key').fill(key);
      await page.getByLabel(/CSRF Token/u).fill(csrf);
      await page.getByRole('button', { name: '解锁', exact: true }).click();
      await page.getByRole('heading', { name: '总览', exact: true }).waitFor();
      await page.locator('.version-picker select').selectOption('prism-local-v4');
      for (const route of routes) {
        await page.evaluate(route => { location.hash = `#${route}`; }, route);
        await page.locator('main h2').first().waitFor();
        // Let the real same-origin queries settle; polling endpoints may remain scheduled.
        await page.waitForTimeout(250);
        const state = await page.evaluate(() => ({
          route: location.hash,
          heading: document.querySelector('main h2')?.textContent,
          width: innerWidth,
          scrollWidth: document.documentElement.scrollWidth,
          alerts: [...document.querySelectorAll('[role="alert"]')].map(node => node.textContent),
          panels: [...document.querySelectorAll('.data-panel--padded')].map(node => ({ label: node.getAttribute('aria-label'), padding: getComputedStyle(node).padding })),
        }));
        for (const panel of state.panels) if (panel.padding.split(' ').some(value => parseFloat(value) < 12)) evidence.errors.push({ width, theme, route, message: `panel padding ${panel.padding}` });
        if (state.scrollWidth > width + 1) evidence.errors.push({ width, theme, route, message: `page overflow ${state.scrollWidth}` });
        const name = route === '/' ? 'overview' : route.slice(1);
        await page.screenshot({ path: `${output}/${width}-${theme}-${name}.png`, fullPage: true });
        evidence.pages.push({ ...state, theme, viewport: [width, height] });
      }
      await context.close();
    }
  }
} finally {
  await browser.close();
  await writeFile(`${output}/audit.json`, JSON.stringify(evidence, null, 2));
}
if (evidence.errors.length) throw new Error(`Production browser audit found ${evidence.errors.length} errors; inspect audit.json`);
console.log(`Production Chromium ${evidence.browser}: ${evidence.pages.length} page views plus 6 unlock views captured`);
