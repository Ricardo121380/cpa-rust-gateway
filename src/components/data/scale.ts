// Chart scale helpers. Pure and DOM-free so the axis maths is testable without
// a browser (src/components/data/scale.test.ts).

/** Round a maximum up to a clean axis top (1 / 2 / 2.5 / 5 × 10^n). */
export function niceCeil(value: number): number {
  if (!Number.isFinite(value) || value <= 0) return 1;
  const exponent = Math.floor(Math.log10(value));
  const magnitude = Math.pow(10, exponent);
  const scaled = value / magnitude;
  const step = scaled <= 1 ? 1 : scaled <= 2 ? 2 : scaled <= 2.5 ? 2.5 : scaled <= 5 ? 5 : 10;
  return step * magnitude;
}

/** Round a tick spacing to 1 / 2 / 2.5 / 5 × 10^n — the only steps that produce
 *  readable axis labels. (niceCeil rounds a TOP; this rounds a STEP.) */
export function niceStep(rough: number): number {
  if (!Number.isFinite(rough) || rough <= 0) return 1;
  const exponent = Math.floor(Math.log10(rough));
  const magnitude = Math.pow(10, exponent);
  const scaled = rough / magnitude;
  const step = scaled <= 1 ? 1 : scaled <= 2 ? 2 : scaled <= 2.5 ? 2.5 : scaled <= 5 ? 5 : 10;
  return step * magnitude;
}

/** Tick values from 0 up to a clean top that covers `max`. The requested count
 *  is a target, not a promise: a clean step wins over an exact tick count,
 *  because 0/100/200/300 reads and 0/62.5/125/187.5 does not. */
export function axisTicks(max: number, count = 4): number[] {
  const step = niceStep(max / Math.max(1, count));
  const steps = Math.max(1, Math.ceil(max / step));
  return Array.from({ length: steps + 1 }, (_, index) => round(index * step));
}

/** Kill float dust so 0.02 * 3 is 0.06, not 0.060000000000000005. */
function round(value: number): number {
  return Number(value.toPrecision(12));
}

/** Approximate rendered width of a label, so an SVG tooltip box can be sized
 *  without measuring the DOM (and without a style attribute). CJK glyphs are
 *  full-width; everything else is roughly 0.58em. */
export function textWidth(text: string, fontSize: number): number {
  let em = 0;
  for (const char of text) {
    em += char.codePointAt(0)! > 0x2e7f ? 1 : 0.58;
  }
  return em * fontSize;
}

export function clamp(value: number, min: number, max: number): number {
  return value < min ? min : value > max ? max : value;
}
