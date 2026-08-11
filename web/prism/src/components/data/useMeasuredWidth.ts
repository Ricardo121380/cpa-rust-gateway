// Render authored SVG at 1:1 with its container.
//
// Why: a fixed viewBox scaled by `width: 100%` scales the TEXT with it, so the
// same 10px axis label renders at 8px inside a half-width card and at 16px in a
// full-width one — three charts on one page, three type sizes. Measuring the
// box and emitting a viewBox in real pixels keeps one type scale everywhere.
import { useEffect, useRef, useState, type RefObject } from "react";

export function useMeasuredWidth(
  fallback: number,
  min = 280,
): [RefObject<HTMLDivElement | null>, number] {
  const ref = useRef<HTMLDivElement | null>(null);
  const [width, setWidth] = useState(fallback);

  useEffect(() => {
    const element = ref.current;
    if (element === null) return;
    const observer = new ResizeObserver((entries) => {
      const measured = entries[0]?.contentRect.width ?? 0;
      if (measured > 0) {
        setWidth(Math.max(min, Math.round(measured)));
      }
    });
    observer.observe(element);
    return () => observer.disconnect();
  }, [min]);

  return [ref, width];
}
