// Production-embed audit. Input is supplied over stdin; secrets never enter artifacts.
import { chromium } from '@playwright/test';
import { mkdir, writeFile } from 'node:fs/promises';
let input = '';
for await (const chunk of process.stdin) input += chunk;
const { base, username = "admin", password, output } = JSON.parse(input);
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
      await page.getByRole('button', { name: '登录', exact: true }).waitFor();
      await page.screenshot({ path: `${output}/${width}-${theme}-unlock.png` });
      await page.getByLabel('账号', { exact: true }).fill(username);
      await page.getByLabel('密码', { exact: true }).fill(password);
      await page.getByRole('button', { name: '登录', exact: true }).click();
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
        if (route === '/audit') {
          const audit = page.getByRole('region', { name: '资源修改审计' });
          const row = audit.locator('tr', { hasText: 'route_candidate_updated' });
          await row.getByRole('button', { name: '查看修改记录' }).click();
          const detail = page.getByRole('dialog', { name: '资源修改记录' });
          await detail.getByText('local-candidate', { exact: true }).waitFor();
          await page.screenshot({ path: `${output}/${width}-${theme}-resource-audit-detail.png` });
          await page.keyboard.press('Escape');
          if (await detail.count()) throw new Error('audit detail did not close on Escape');
          if (await page.locator(':focus').textContent() !== '查看修改记录') throw new Error('audit focus was not restored');
        }

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
