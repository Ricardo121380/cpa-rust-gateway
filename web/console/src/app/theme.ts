// Light/dark theme. Preference is kept in memory for the session (the console stores nothing in the browser)
// and starts from the OS setting. The switch reveals the new theme as a circle from the pointer.
import { useSyncExternalStore } from "react";

let theme: "light" | "dark" = matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
document.documentElement.classList.toggle("dark", theme === "dark");
const subs = new Set<() => void>();

export function useTheme() {
  return useSyncExternalStore((s) => { subs.add(s); return () => subs.delete(s); }, () => theme);
}

export function toggleTheme(at?: { clientX: number; clientY: number }) {
  const next = theme === "dark" ? "light" : "dark";
  const apply = () => { theme = next; document.documentElement.classList.toggle("dark", next === "dark"); subs.forEach((s) => s()); };
  const d = document as Document & { startViewTransition?: (cb: () => void) => { ready: Promise<void>; finished: Promise<void> } };
  if (!d.startViewTransition || matchMedia("(prefers-reduced-motion: reduce)").matches || document.hidden) return apply();
  const x = at?.clientX ?? innerWidth - 40, y = at?.clientY ?? 40;
  const r = Math.hypot(Math.max(x, innerWidth - x), Math.max(y, innerHeight - y));
  const vt = d.startViewTransition(apply);
  vt.ready.then(() => document.documentElement.animate({ clipPath: [`circle(0 at ${x}px ${y}px)`, `circle(${r}px at ${x}px ${y}px)`] }, { duration: 520, easing: "cubic-bezier(.22,1,.36,1)", pseudoElement: "::view-transition-new(root)" })).catch(() => {});
  vt.finished.catch(() => {});
}
