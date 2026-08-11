// Coarse clock for time-range query keys.
//
// Why this exists: a range whose `to_ms` is `Date.now()` computed during
// render changes on EVERY render, so any query key derived from it is new
// every render — infinite-query pages reset (pagination silently stops
// growing) and polling queries thrash. Quantising "now" to a coarse tick
// keeps keys stable between ticks while still advancing the window.
import { useEffect, useState } from "react";

export function useNowTick(periodMs = 60_000): number {
  const [now, setNow] = useState(() => Math.floor(Date.now() / periodMs) * periodMs);
  useEffect(() => {
    const timer = window.setInterval(() => {
      setNow(Math.floor(Date.now() / periodMs) * periodMs);
    }, periodMs);
    return () => window.clearInterval(timer);
  }, [periodMs]);
  return now;
}
