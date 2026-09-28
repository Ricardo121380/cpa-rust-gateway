import { useId, useLayoutEffect, useRef } from "react";
import { useAccessibilityStore } from "../../app/themeStore";
import { useMediaQuery } from "../../utils/useMediaQuery";
import "./selection-indicator.css";

type Bounds = Readonly<{ x: number; y: number; width: number; height: number }>;

/** Decorative geometry only; the caller owns routing, selection and focus. */
export function SelectionIndicator({ activeKey, selector, layoutKey }: Readonly<{
  activeKey: string | number;
  selector: string;
  layoutKey?: boolean;
}>) {
  const svgRef = useRef<SVGSVGElement>(null);
  const rectRef = useRef<SVGRectElement>(null);
  const current = useRef<Bounds | null>(null);
  const previousKey = useRef(activeKey);
  const gradient = useId();
  const sessionReduced = useAccessibilityStore(state => state.motion);
  const systemReduced = useMediaQuery("(prefers-reduced-motion: reduce)");
  const forcedColors = useMediaQuery("(forced-colors: active)");
  const reduced = sessionReduced || systemReduced;
  const preferences = useRef({ reduced, forcedColors });
  const placeRef = useRef<((animate: boolean) => void) | null>(null);

  useLayoutEffect(() => { preferences.current = { reduced, forcedColors }; }, [reduced, forcedColors]);

  useLayoutEffect(() => {
    const svg = svgRef.current;
    const rect = rectRef.current;
    const host = svg?.parentElement;
    if (!svg || !rect || !host) return;
    let animation = 0;
    let layout = 0;

    // SVG attributes preserve the shipped style-src 'self' policy.
    const draw = (bounds: Bounds) => {
      current.current = bounds;
      for (const key of ["x", "y", "width", "height"] as const) {
        rect.setAttribute(key, bounds[key].toFixed(2));
      }
    };
    const place = (animate: boolean) => {
      cancelAnimationFrame(animation);
      const active = host.querySelector<HTMLElement>(selector);
      const box = active?.getBoundingClientRect();
      if (preferences.current.forcedColors || !box || box.width === 0 || box.height === 0) {
        current.current = null;
        delete host.dataset.selectionReady;
        delete svg.dataset.visible;
        return;
      }
      const origin = svg.getBoundingClientRect();
      const target = { x: box.left - origin.left, y: box.top - origin.top, width: box.width, height: box.height };
      const duration = preferences.current.reduced ? 0 : parseFloat(getComputedStyle(host).getPropertyValue("--motion-select")) || 0;
      const from = current.current;
      rect.setAttribute("rx", String(parseFloat(getComputedStyle(active!).borderRadius) || 0));
      host.dataset.selectionReady = "true";
      svg.dataset.visible = "true";
      if (!animate || !from || duration === 0) {
        draw(target);
        return;
      }
      const start = performance.now();
      const frame = (at: number) => {
        const progress = Math.min(1, (at - start) / duration);
        const eased = 1 - (1 - progress) ** 3;
        draw({
          x: from.x + (target.x - from.x) * eased,
          y: from.y + (target.y - from.y) * eased,
          width: from.width + (target.width - from.width) * eased,
          height: from.height + (target.height - from.height) * eased,
        });
        if (progress < 1) animation = requestAnimationFrame(frame);
      };
      animation = requestAnimationFrame(frame);
    };
    const reflow = () => {
      if (layout) return;
      layout = requestAnimationFrame(() => { layout = 0; place(false); });
    };
    placeRef.current = place;
    place(false);
    const resize = new ResizeObserver(reflow);
    resize.observe(host);
    for (const child of host.children) if (child !== svg) resize.observe(child);
    const content = new MutationObserver(reflow);
    content.observe(host, { childList: true, characterData: true, subtree: true });
    host.addEventListener("scroll", reflow, { passive: true });
    return () => {
      cancelAnimationFrame(animation);
      cancelAnimationFrame(layout);
      resize.disconnect();
      content.disconnect();
      host.removeEventListener("scroll", reflow);
      delete host.dataset.selectionReady;
      delete svg.dataset.visible;
      placeRef.current = null;
    };
  }, [selector]);

  useLayoutEffect(() => {
    const changed = previousKey.current !== activeKey;
    previousKey.current = activeKey;
    placeRef.current?.(changed);
  }, [activeKey, layoutKey, reduced, forcedColors]);

  return <svg ref={svgRef} className="selection-indicator" aria-hidden="true" focusable="false">
    <defs><linearGradient id={gradient} x1="0" y1="0" x2="1" y2="1"><stop offset="0%" className="selection-pearl-start"/><stop offset="100%" className="selection-pearl-end"/></linearGradient></defs>
    <rect ref={rectRef} fill={`url(#${gradient})`}/>
  </svg>;
}
