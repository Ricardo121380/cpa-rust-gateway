// Global toast surface (beUI animated-toast-stack). Callers use `toast(...)` from anywhere.
import { useSyncExternalStore } from "react";
import { AnimatedToastStack, type AnimatedToast, type ToastInput } from "@/components/motion/animated-toast-stack";

let toasts: AnimatedToast[] = [];
const subs = new Set<() => void>();
const emit = () => subs.forEach((s) => s());
let seq = 0;

export function toast(input: ToastInput) {
  const t: AnimatedToast = { duration: 3800, dismissible: true, ...input, id: input.id ?? `t${++seq}`, createdAt: Date.now() };
  toasts = [t, ...toasts.filter((x) => x.id !== t.id)].slice(0, 4);
  emit();
  if (t.duration && t.status !== "loading") setTimeout(() => dismiss(t.id), t.duration);
  return t.id;
}
export function dismiss(id: string) { toasts = toasts.filter((t) => t.id !== id); emit(); }

export function Toasts() {
  const list = useSyncExternalStore((s) => { subs.add(s); return () => subs.delete(s); }, () => toasts);
  return <AnimatedToastStack toasts={list} onDismiss={dismiss} position={"bottom-center" as never} fixed />;
}
