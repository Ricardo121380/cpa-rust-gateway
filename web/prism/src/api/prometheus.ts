// Prometheus text-exposition parser for GET /admin/observability/metrics.
//
// The gateway serves `text/plain; version=0.0.4` from a frozen, bounded
// counter set with no request- or target-scoped labels
// (crates/gateway-observability/src/telemetry.rs::render_prometheus), so this
// only has to read what that renderer can emit:
//
//   # HELP name help text
//   # TYPE name counter
//   name{label="value",other="v"} 12
//   name 7
//
// It is still a parse of server output, so malformed lines are dropped rather
// than trusted: a NaN reaching a StatTile would render as "NaN".

export type PromSample = Readonly<{
  name: string;
  labels: Readonly<Record<string, string>>;
  value: number;
}>;

const SAMPLE = /^([a-zA-Z_:][a-zA-Z0-9_:]*)(?:\{([^}]*)\})?[ \t]+(\S+)/u;
const LABEL = /([a-zA-Z_][a-zA-Z0-9_]*)="((?:[^"\\]|\\.)*)"/gu;

function readLabels(raw: string | undefined): Record<string, string> {
  const labels: Record<string, string> = {};
  if (raw === undefined) {
    return labels;
  }
  LABEL.lastIndex = 0;
  for (let match = LABEL.exec(raw); match !== null; match = LABEL.exec(raw)) {
    const name = match[1];
    if (name === undefined) {
      continue;
    }
    labels[name] = (match[2] ?? "").replace(/\\(.)/gu, (_whole, char: string) =>
      char === "n" ? "\n" : char,
    );
  }
  return labels;
}

export function parsePrometheus(text: string): readonly PromSample[] {
  const samples: PromSample[] = [];
  for (const line of text.split("\n")) {
    const trimmed = line.trim();
    if (trimmed.length === 0 || trimmed.startsWith("#")) {
      continue;
    }
    const match = SAMPLE.exec(trimmed);
    const name = match?.[1];
    if (match === null || name === undefined) {
      continue;
    }
    const value = Number(match[3]);
    if (!Number.isFinite(value)) {
      continue;
    }
    samples.push({ name, labels: readLabels(match[2]), value });
  }
  return samples;
}

/**
 * Sums every sample with this name whose labels contain the given subset.
 * With no labels it totals the family: `pick(s, "…events_total")` adds up
 * every `kind`.
 */
export function pick(
  samples: readonly PromSample[],
  name: string,
  labels: Readonly<Record<string, string>> = {},
): number {
  const wanted = Object.entries(labels);
  let total = 0;
  for (const sample of samples) {
    if (sample.name !== name) {
      continue;
    }
    if (wanted.every(([key, value]) => sample.labels[key] === value)) {
      total += sample.value;
    }
  }
  return total;
}
