// Objective legibility probe for the glass panes.
//
// "Does content ghost through the pane?" is a contrast question, so measure it
// instead of squinting: sample the pane's interior, and report the RMS
// luminance deviation. A pane sitting over dense 13px table text scores high
// (the glyphs survive); a pane that has properly veiled its backdrop scores
// low. The ambient-only reading is the floor — you can never do better than
// the gradient that is genuinely behind the pane.
//
// usage: node proto/measure.mjs
import { chromium } from "@playwright/test";
import { pathToFileURL } from "node:url";
import { resolve } from "node:path";

const url = pathToFileURL(resolve("proto/glass-lab.html")).href;
const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 1 });
await page.goto(url);
await page.waitForFunction(() => window.__labReady === true);
await page.evaluate(() => { document.documentElement.dataset.lab = "off"; });

// Interior sample box, inset from the pane's own glyphs and from its rim, so
// the number reflects the BACKDROP only. The dock's own text lives in the
// middle band, so sample the strip just below it.
async function sample(sel, insetTop, insetBottom) {
  return page.evaluate(async ([s, it, ib]) => {
    const el = document.querySelector(s);
    const r = el.getBoundingClientRect();
    const x0 = Math.round(r.left + 26), x1 = Math.round(r.right - 26);
    const y0 = Math.round(r.top + it), y1 = Math.round(r.bottom - ib);
    // html2canvas-free readback: use the compositor via drawImage on a
    // same-origin snapshot is not available, so ask the page for a
    // devicePixelRatio-1 screenshot region through CSS paint... not possible.
    // Instead return the box; the node side does the pixel work.
    return { x: x0, y: y0, width: Math.max(1, x1 - x0), height: Math.max(1, y1 - y0) };
  }, [sel, insetTop, insetBottom]);
}

// Decode in-browser: screenshot -> data URL -> canvas -> pixel stats.
async function contrastOf(box) {
  const shot = await page.screenshot({ clip: box });
  const b64 = shot.toString("base64");
  return page.evaluate(async ([data]) => {
    const img = new Image();
    img.src = "data:image/png;base64," + data;
    await img.decode();
    const c = document.createElement("canvas");
    c.width = img.naturalWidth; c.height = img.naturalHeight;
    const ctx = c.getContext("2d", { willReadFrequently: true });
    ctx.drawImage(img, 0, 0);
    const d = ctx.getImageData(0, 0, c.width, c.height).data;
    const lum = [];
    for (let i = 0; i < d.length; i += 4) {
      lum.push(0.2126 * d[i] + 0.7152 * d[i + 1] + 0.0722 * d[i + 2]);
    }
    const mean = lum.reduce((a, b) => a + b, 0) / lum.length;
    const rms = Math.sqrt(lum.reduce((a, b) => a + (b - mean) ** 2, 0) / lum.length);
    // high-frequency energy: |pixel - 3px-boxcar| catches GLYPHS specifically,
    // which a smooth ambient gradient does not trigger
    let hf = 0, n = 0;
    for (let y = 0; y < c.height; y++) {
      for (let x = 3; x < c.width - 3; x++) {
        const i = y * c.width + x;
        const local = (lum[i - 3] + lum[i] + lum[i + 3]) / 3;
        hf += Math.abs(lum[i] - local); n++;
      }
    }
    return { mean: +mean.toFixed(1), rms: +rms.toFixed(2), hf: +(hf / n).toFixed(3) };
  }, [b64]);
}

async function setup(theme, treatment, target, vars) {
  await page.evaluate(([t, tr, sel, v]) => {
    const r = document.documentElement;
    r.dataset.theme = t; r.dataset.treatment = tr;
    const node = sel === ":root" ? r : document.querySelector(sel);
    for (const name of (node.getAttribute("data-swept") ?? "").split(",")) {
      if (name) node.style.removeProperty(name);
    }
    for (const [k, val] of Object.entries(v)) node.style.setProperty(k, val);
    node.setAttribute("data-swept", Object.keys(v).join(","));
    window.dispatchEvent(new Event("resize"));
  }, [theme, treatment, target, vars]);
  await page.waitForTimeout(380);
}
async function scrollTo(y) {
  await page.evaluate((v) => {
    const c = document.querySelector(".canvas");
    c.scrollTo({ top: v, behavior: "instant" });
    c.dispatchEvent(new Event("scroll"));
  }, y);
  await page.waitForTimeout(300);
}

const VARIANTS = [
  ["rim-blur 0 (sharp) ", { "--lens-rim-blur": "0" }],
  ["rim-blur 2         ", { "--lens-rim-blur": "2" }],
  ["rim-blur 3         ", { "--lens-rim-blur": "3" }],
  ["rim-blur 5         ", { "--lens-rim-blur": "5" }],
  ["rim-blur 8         ", { "--lens-rim-blur": "8" }],
  ["rim-blur 12        ", { "--lens-rim-blur": "12" }],
];

console.log("dock over dense table rows — interior backdrop contrast (light)");
console.log("variant                mean    rms     hf(glyph energy)");
await setup("light", "c", ".dock", {});
await scrollTo(470);
const box = await sample(".dock", 8, 2);
for (const [name, vars] of VARIANTS) {
  await setup("light", "c", ".dock", vars);
  await scrollTo(470);
  const s = await contrastOf(box);
  console.log(`${name}  ${String(s.mean).padStart(6)} ${String(s.rms).padStart(7)} ${String(s.hf).padStart(8)}`);
}

console.log("\nfloor: same pane parked over pure ambient (no content behind)");
await setup("light", "c", ".dock", {});
await scrollTo(100000);
console.log("  ambient-only      ", await contrastOf(box));

await browser.close();
