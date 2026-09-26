// EgoLite fixture-only UI regression comparison; no production or Provider requests.
// PRISM_M3={spaceId,receipts,baselineOrigin,candidateOrigin}; same p2, viewport and fixtures.
const fs = await import("node:fs/promises");
const path = await import("node:path");
const { spaceId, receipts, baselineOrigin, candidateOrigin } = globalThis.PRISM_M3;
const page = (await taskSpace(spaceId)).page("p2");
const results = {};
for (const [label, value] of [["baseline", baselineOrigin], ["candidate", candidateOrigin]]) {
  const origin = new URL(value);
  if (origin.hostname !== "127.0.0.1" || origin.protocol !== "http:") throw Error("Loopback fixture required");
  await page.goto(origin.origin + "/#/unlock");
  await page.reload();
  await page.cdp("Page.bringToFront", {});
  await page.fill('input[name="password"]', "Prism-demo-2026");
  await page.keyboard.press("Escape");
  await page.click('loc=role:heading[name="管理员登录"]');
  await page.click('loc=role:button[name="登录"]');
  await page.waitForSelector("main.canvas", { state: "visible" });
  await page.cdp("Emulation.setDeviceMetricsOverride", { width: 1440, height: 900, deviceScaleFactor: 1, mobile: false });
  await page.goto(origin.origin + "/#/settings");
  await page.click('loc=role:radio[name="浅色"]');
  const samples = { navigation_ms: [], dialog_ms: [] };
  for (let round = 0; round < 6; round++) {
    await page.goto(origin.origin + "/#/monitoring");
    await page.waitForSelector("main.canvas h2", { state: "visible" });
    for (const mode of ["navigation", "dialog"]) {
      await page.evaluate((kind) => {
        window.__m3Timing = null;
        document.addEventListener("click", () => {
          const start = performance.now();
          let frames = 0;
          const observe = async () => {
            const ready = kind === "dialog"
              ? !!document.querySelector('[role="dialog"]')
              : [...document.querySelectorAll("main.canvas h2")].some(e => e.textContent === "账号管理")
                && !!document.querySelector('[aria-label="搜索账号"]')
                && ![...document.querySelectorAll('main.canvas [role="status"]')].some(e => /读取|加载/.test(e.textContent));
            if (!ready && ++frames < 600) { requestAnimationFrame(observe); return; }
            if (!ready) { window.__m3Timing = { error: "render timeout" }; return; }
            await Promise.all(document.getAnimations().filter(a => a.effect?.getComputedTiming().iterations !== Infinity).map(a => a.finished.catch(() => {})));
            await new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)));
            window.__m3Timing = { ms: performance.now() - start };
          };
          requestAnimationFrame(observe);
        }, { once: true, capture: true });
      }, mode);
      await page.click(mode === "navigation" ? 'loc=css:a[href="#/accounts"]' : 'loc=role:button[name="授权 / 导入账号"]');
      await page.waitForFunction(() => window.__m3Timing !== null, undefined, { timeout: 15000 });
      const result = await page.evaluate(() => window.__m3Timing);
      if (result.error) throw Error(result.error);
      if (round > 0) samples[mode + "_ms"].push(result.ms);
    }
    await page.keyboard.press("Escape");
    await page.waitForFunction(() => !document.querySelector('[role="dialog"]'));
  }
  results[label] = samples;
}
const median = values => [...values].sort((a, b) => a - b)[2];
const comparisons = Object.keys(results.baseline).map(metric => {
  const baseline = median(results.baseline[metric]);
  const candidate = median(results.candidate[metric]);
  return { metric, baseline_median_ms: baseline, candidate_median_ms: candidate, delta_ms: candidate - baseline, regression: candidate > baseline * 1.1 && candidate > baseline + 50 };
});
const receipt = { browser: "EgoLite", fixtureOnly: true, viewport: "1440x900", theme: "light", warmup: 1, samples: 5, timing: "browser click to settled render including finite animations; Vite fixture, not production network latency or Core Web Vitals", threshold: "regression requires both >10% and >50ms", results, comparisons, passed: comparisons.every(row => !row.regression) };
await fs.writeFile(path.join(receipts, "ui-timing.json"), JSON.stringify(receipt, null, 2));
console.log(receipt);
if (!receipt.passed) throw Error("UI interaction regression requires investigation");
