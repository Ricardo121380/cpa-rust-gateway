// PORT 3/3 (companion) — new file: src/components/glass/PrismLens.tsx
//
// Mount ONCE, inside .shell, above the panes. It owns:
//   * the three <filter> definitions consumed by glass.css
//   * the ambient grain filter
//   * the displacement maps (rebuilt only when a pane's box actually changes)
//   * the runtime capability probe that writes html[data-lens]
//   * the data-over="content|ambient" probe that drives the adaptive shadow
//
// CSP / scripts/check.mjs compliance:
//   - no style attribute is ever written; only SVG presentation ATTRIBUTES
//     (href, width, height, scale, stdDeviation, values)
//   - the displacement map is a canvas data: URL, which the shipped CSP allows
//     via `img-src 'self' data:` — verified in Chromium against the real
//     policy string, the map renders and no violation is reported
//   - no browser storage, no fetch
import { useEffect, useRef } from "react";

const N = 200; // profile samples along one radius

/** Apple's squircle profile: y = (1-(1-x)^4)^(1/4).
 *  Softer flat->curve transition than a circular arc, so the refraction
 *  gradient stays smooth when the shape is stretched into a long rectangle.
 *  A plain circle produced a visible hard seam where the bezel met the flat
 *  interior on the 1416px-wide topbar. */
const squircle = (x: number): number => Math.pow(1 - Math.pow(1 - x, 4), 0.25);

/** Snell–Descartes along one radius: air (n=1) -> glass (n=ior).
 *  t = 0 at the outer edge, 1 at the end of the bezel. Magnitudes come back
 *  normalised to 1, and `max` is the px scale to hand to feDisplacementMap —
 *  which is exactly what the `scale` attribute means. */
function profile(bezel: number, thickness: number, ior: number) {
  const mags: number[] = [];
  for (let i = 0; i < N; i += 1) {
    const t = i / (N - 1);
    const d = 1e-3;
    const slope =
      ((squircle(Math.min(1, t + d)) - squircle(Math.max(0, t - d))) / (2 * d)) *
      (thickness / bezel);
    const theta1 = Math.atan(slope);
    const theta2 = Math.asin(Math.max(-1, Math.min(1, Math.sin(theta1) / ior)));
    // the (1 - .55*height) term fades the bend as the glass flattens out
    mags.push(Math.tan(theta1 - theta2) * thickness * (1 - squircle(t) * 0.55));
  }
  const max = Math.max(...mags, 1e-6);
  return { mags: mags.map((m) => m / max), max };
}

/** R = x displacement, G = y displacement, B = bezel coverage (the rim mask —
 *  feDisplacementMap ignores B and A, so the mask rides in the same image and
 *  costs nothing). 128 is neutral in R/G. */
function buildMap(
  w: number, h: number, radius: number,
  bezel: number, thickness: number, ior: number,
): { url: string; scale: number } {
  const c = document.createElement("canvas");
  c.width = Math.max(2, Math.round(w));
  c.height = Math.max(2, Math.round(h));
  const ctx = c.getContext("2d");
  if (ctx === null) return { url: "", scale: 0 };

  // CLAMP THE BEZEL TO THE PANE'S SHORT SIDE.
  // The rim band is the SHARP, displaced backdrop composited OVER the frost.
  // A bezel wider than half the short side therefore covers the whole pane,
  // the frost never shows, and what looks like glass is really a clear window
  // that leaks every glyph underneath at full contrast.
  // Measured on this shell at the nominal 26px bezel:
  //   rail   196x810  rim =   6% of height   correct
  //   topbar 1416x54  rim =  96% of height   BROKEN
  //   dock    545x51  rim = 100% of height   BROKEN
  // Fixing this dropped measured glyph energy under the dock from 4.28 to
  // 1.49, against an ambient-only floor of 1.42.
  const bez = Math.max(4, Math.min(bezel, Math.min(c.width, c.height) * 0.3));

  const img = ctx.createImageData(c.width, c.height);
  const { mags, max } = profile(bez, thickness, ior);
  const r = Math.min(radius, c.width / 2, c.height / 2);

  for (let y = 0; y < c.height; y += 1) {
    for (let x = 0; x < c.width; x += 1) {
      const px = x + 0.5;
      const py = y + 0.5;
      // signed distance to a rounded rect + its outward normal
      const cx = Math.min(Math.max(px, r), c.width - r);
      const cy = Math.min(Math.max(py, r), c.height - r);
      let nx = px - cx;
      let ny = py - cy;
      let dist: number;
      const len = Math.hypot(nx, ny);
      if (len < 1e-6) {
        const dl = px, dr = c.width - px, dt = py, db = c.height - py;
        dist = Math.min(dl, dr, dt, db);
        if (dist === dl) { nx = -1; ny = 0; }
        else if (dist === dr) { nx = 1; ny = 0; }
        else if (dist === dt) { nx = 0; ny = -1; }
        else { nx = 0; ny = 1; }
      } else {
        dist = r - len; nx /= len; ny /= len;
      }
      const t = Math.min(1, Math.max(0, dist / bez));
      const mag = mags[Math.min(N - 1, Math.round(t * (N - 1)))] ?? 0;
      // rim coverage: 1 across the bezel, smoothstepped to 0 just inside it
      const u = Math.min(1, Math.max(0, 1 - t));
      const cover = u * u * (3 - 2 * u);
      const i = (y * c.width + x) * 4;
      img.data[i] = Math.round(128 + nx * mag * 127);
      img.data[i + 1] = Math.round(128 + ny * mag * 127);
      img.data[i + 2] = Math.round(cover * 255);
      img.data[i + 3] = 255;
    }
  }
  ctx.putImageData(img, 0, 0);
  return { url: c.toDataURL("image/png"), scale: max };
}

const PANES = [
  { sel: ".topbar", id: "prism-lens-topbar" },
  { sel: ".rail", id: "prism-lens-rail" },
  { sel: ".dock", id: "prism-lens-dock" },
] as const;

function num(el: Element, name: string, fallback: number): number {
  const v = Number.parseFloat(getComputedStyle(el).getPropertyValue(name));
  return Number.isFinite(v) ? v : fallback;
}

/** ms from a CSS duration token ("600ms" / "0.6s" / "0ms"). */
function durationMs(el: Element, name: string): number {
  const raw = getComputedStyle(el).getPropertyValue(name).trim();
  const v = Number.parseFloat(raw);
  if (!Number.isFinite(v)) return 0;
  return raw.endsWith("ms") ? v : v * 1000;
}

/** The four numbers that carry material semantics on the lens path. */
type Dynamics = { scale: number; frost: number; rim: number; sat: number };

function applyDynamics(f: Element, d: Dynamics): void {
  // negative scale = sample OUTWARD (physically correct convex lens)
  f.querySelector("feDisplacementMap")?.setAttribute("scale", (-d.scale).toFixed(2));
  const blurs = f.querySelectorAll("feGaussianBlur");
  blurs[0]?.setAttribute("stdDeviation", d.frost.toFixed(2));
  blurs[1]?.setAttribute("stdDeviation", d.rim.toFixed(2));
  f.querySelector('feColorMatrix[type="saturate"]')?.setAttribute("values", d.sat.toFixed(3));
}

/** cubic-bezier(0.32, 0.72, 0, 1) — the shell's --ease, sampled by bisection.
 *  Hard-coded rather than parsed: this is the one easing the design system
 *  defines, and the CSS transition on the layered path uses the same curve, so
 *  both paths anneal on the same clock AND the same shape. */
function ease(t: number): number {
  const bx = (u: number): number => 3 * u * (1 - u) ** 2 * 0.32 + u ** 3;
  const by = (u: number): number => 3 * u * (1 - u) ** 2 * 0.72 + 3 * u ** 2 * (1 - u) + u ** 3;
  let lo = 0;
  let hi = 1;
  for (let i = 0; i < 24; i += 1) {
    const mid = (lo + hi) / 2;
    if (bx(mid) < t) lo = mid;
    else hi = mid;
  }
  return by((lo + hi) / 2);
}

/** Live tween per filter id, so a second material change mid-anneal replaces
 *  the first instead of racing it. */
const tweens = new Map<string, number>();

/** SVG filter primitives are attributes, not animatable CSS properties, so the
 *  `transition: backdrop-filter` that anneals the layered path is a no-op here:
 *  under `backdrop-filter: url(#...)` a draft->active publish snapped between
 *  two frost levels in a single frame. Tween the primitive values on the same
 *  duration and easing token instead. */
function annealTo(f: Element, id: string, from: Dynamics, to: Dynamics, ms: number): void {
  const running = tweens.get(id);
  if (running !== undefined) cancelAnimationFrame(running);
  const t0 = performance.now();
  const step = (now: number): void => {
    const p = Math.min(1, (now - t0) / ms);
    const k = ease(p);
    applyDynamics(f, {
      scale: from.scale + (to.scale - from.scale) * k,
      frost: from.frost + (to.frost - from.frost) * k,
      rim: from.rim + (to.rim - from.rim) * k,
      sat: from.sat + (to.sat - from.sat) * k,
    });
    if (p < 1) tweens.set(id, requestAnimationFrame(step));
    else tweens.delete(id);
  };
  tweens.set(id, requestAnimationFrame(step));
}

function updatePane({ sel, id }: { sel: string; id: string }): void {
  const el = document.querySelector(sel);
  const f = document.getElementById(id);
  if (el === null || f === null) return;
  const rect = el.getBoundingClientRect();
  if (rect.width < 4 || rect.height < 4) return;

  const cs = getComputedStyle(el);
  const radius = Number.parseFloat(cs.borderTopLeftRadius) || 20;
  const bezel = num(el, "--lens-bezel", 26);
  const thickness = num(el, "--lens-thickness", 34);
  const ior = num(el, "--lens-ior", 1.5);
  const gain = num(el, "--lens-gain", 1);

  const key = [
    Math.round(rect.width), Math.round(rect.height),
    radius, bezel, thickness, ior,
  ].join("/");

  if (f.dataset.key !== key) {
    const { url, scale } = buildMap(rect.width, rect.height, radius, bezel, thickness, ior);
    f.dataset.key = key;
    f.dataset.scale = String(scale);
    const im = f.querySelector("feImage");
    im?.setAttribute("href", url);
    im?.setAttribute("width", String(Math.round(rect.width)));
    im?.setAttribute("height", String(Math.round(rect.height)));
  }

  // --lens-frost / --lens-sat are what data-material actually overrides, so they
  // must be re-read on every call, not only when the map is rebuilt.
  const target: Dynamics = {
    scale: Number.parseFloat(f.dataset.scale ?? "0") * gain,
    frost: num(el, "--lens-frost", 10),
    rim: num(el, "--lens-rim-blur", 3),
    sat: num(el, "--lens-sat", 1.8),
  };

  const prev = f.dataset.dyn;
  const next = `${target.scale}/${target.frost}/${target.rim}/${target.sat}`;
  if (prev === next) return;
  f.dataset.dyn = next;

  const anneal = durationMs(el, "--dur-anneal");
  const from = prev?.split("/").map(Number);
  if (
    from === undefined || from.length !== 4 ||
    from.some((n) => !Number.isFinite(n)) || anneal <= 0
  ) {
    applyDynamics(f, target);
    return;
  }
  annealTo(
    f,
    id,
    { scale: from[0] ?? 0, frost: from[1] ?? 0, rim: from[2] ?? 0, sat: from[3] ?? 0 },
    target,
    anneal,
  );
}

/** WWDC25 deepens the shadow while real content is behind the pane.
 *  One elementFromPoint per pane per scroll frame — cheap. */
function updateOver(): void {
  for (const { sel } of PANES) {
    const el = document.querySelector(sel);
    if (el === null) continue;
    const r = el.getBoundingClientRect();
    const probe = document.elementFromPoint(
      Math.min(window.innerWidth - 2, r.left + r.width / 2),
      Math.max(2, Math.min(window.innerHeight - 2, r.top + r.height + 6)),
    );
    const over = probe !== null && probe.closest(".canvas") !== null;
    (el as HTMLElement).dataset.over = over ? "content" : "ambient";
  }
}

const LENS_CHAIN = (
  <>
    <feImage result="map" preserveAspectRatio="none" x="0" y="0" width="10" height="10" />
    {/* B channel -> alpha: the bezel band, so the sharp rim is clipped to it */}
    <feColorMatrix
      in="map" result="rimmask" type="matrix"
      values="0 0 0 0 1  0 0 0 0 1  0 0 0 0 1  0 0 1 0 0"
    />
    {/* the frosted interior */}
    <feGaussianBlur in="SourceGraphic" stdDeviation="10" result="frost" />
    {/* The rim is a LENS, not a hole: pre-blur it slightly or the bezel shows a
        pin-sharp compressed image of whatever is under it. Keep this small —
        at stdDeviation >= 20 the bend disappears entirely. */}
    <feGaussianBlur in="SourceGraphic" stdDeviation="3" result="rimsrc" />
    <feDisplacementMap
      in="rimsrc" in2="map" scale="-38"
      xChannelSelector="R" yChannelSelector="G" result="bent"
    />
    <feComposite in="bent" in2="rimmask" operator="in" result="rim" />
    <feMerge result="stack">
      <feMergeNode in="frost" />
      <feMergeNode in="rim" />
    </feMerge>
    <feColorMatrix in="stack" type="saturate" values="1.8" result="sat" />
    <feComponentTransfer in="sat">
      <feFuncR type="linear" slope="1.06" intercept="0" />
      <feFuncG type="linear" slope="1.06" intercept="0" />
      <feFuncB type="linear" slope="1.06" intercept="0" />
    </feComponentTransfer>
  </>
);

export function PrismLens(): React.ReactElement {
  const ref = useRef<SVGSVGElement>(null);

  useEffect(() => {
    const root = document.documentElement;

    // `@supports (backdrop-filter: url(#x))` is TRUE in Firefox and Safari and
    // then renders NOTHING, so CSS alone cannot gate this.
    const isFirefox = CSS.supports("-moz-appearance", "none");
    const isSafari = CSS.supports("background", "-webkit-named-image(i)");
    const parsesUrl =
      CSS.supports("backdrop-filter", "url(#prism-lens-rail)") ||
      CSS.supports("-webkit-backdrop-filter", "url(#prism-lens-rail)");
    root.dataset.lens = parsesUrl && !isFirefox && !isSafari ? "on" : "off";

    const updateAll = (): void => { PANES.forEach(updatePane); };
    updateAll();
    updateOver();

    // Mutation storms (a table rendering 200 rows) must not run the pane sweep
    // once per record: collapse to one sweep per frame.
    let queued = 0;
    const scheduleAll = (): void => {
      if (queued !== 0) return;
      queued = requestAnimationFrame(() => {
        queued = 0;
        updateAll();
      });
    };

    // The body ResizeObserver alone is not enough. Every pane is position:fixed,
    // so none of them changes the body's box: the dock (mounted later, when a
    // draft is selected) never got a map at all, its feImage kept href="" and it
    // rendered as a clear window instead of glass. A MutationObserver on .shell
    // catches the mount, and data-material changes on an existing pane, which is
    // what drives the anneal. The filter is narrow on purpose — the lens <defs>
    // live inside .shell too, and observing the attributes this code writes
    // (href/scale/stdDeviation/values) would make it retrigger itself.
    const mo = new MutationObserver(scheduleAll);
    const shell = document.querySelector(".shell");
    if (shell !== null) {
      mo.observe(shell, {
        childList: true,
        subtree: true,
        attributes: true,
        attributeFilter: ["data-material", "class"],
      });
    }

    const ro = new ResizeObserver(scheduleAll);
    ro.observe(document.body);
    for (const { sel } of PANES) {
      const el = document.querySelector(sel);
      if (el !== null) ro.observe(el);
    }
    window.addEventListener("resize", updateAll);

    const canvas = document.querySelector(".canvas");
    const onScroll = (): void => { updateOver(); };
    canvas?.addEventListener("scroll", onScroll, { passive: true });

    return () => {
      mo.disconnect();
      ro.disconnect();
      if (queued !== 0) cancelAnimationFrame(queued);
      for (const id of tweens.values()) cancelAnimationFrame(id);
      tweens.clear();
      window.removeEventListener("resize", updateAll);
      canvas?.removeEventListener("scroll", onScroll);
    };
  }, []);

  return (
    <svg ref={ref} className="lens-defs" aria-hidden="true" focusable="false">
      <defs>
        <filter id="prism-grain" x="0%" y="0%" width="100%" height="100%">
          <feTurbulence
            type="fractalNoise" baseFrequency="0.82" numOctaves={3}
            seed={7} stitchTiles="stitch" result="n"
          />
          <feColorMatrix in="n" type="saturate" values="0" result="g" />
          <feComponentTransfer in="g">
            <feFuncA type="linear" slope="0.42" intercept="0" />
          </feComponentTransfer>
        </filter>

        {PANES.map(({ id }) => (
          <filter
            key={id} id={id}
            x="0%" y="0%" width="100%" height="100%"
            colorInterpolationFilters="sRGB"
          >
            {LENS_CHAIN}
          </filter>
        ))}
      </defs>
    </svg>
  );
}
