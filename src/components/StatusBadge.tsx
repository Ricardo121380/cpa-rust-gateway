// Closed-vocabulary status badge: color + dot + text, never color alone
// (docs/07 §8.6). Tones map to the reserved status pool.
import type { ReactNode } from "react";

export type BadgeTone = "good" | "warn" | "serious" | "critical" | "tint" | "muted";

const TONE_FOR: Record<string, BadgeTone> = {
  // config version
  draft: "muted",
  active: "good",
  archived: "muted",
  // client key (+derived)
  disabled: "muted",
  revoked: "critical",
  expired: "serious",
  // runtime availability
  available: "good",
  cooldown: "warn",
  circuit_open: "serious",
  quota_blocked: "serious",
  credential_forbidden: "critical",
  recovery_required: "tint",
  // catalog freshness
  fresh: "good",
  stale: "warn",
  missing: "muted",
};

export function toneFor(status: string): BadgeTone {
  return TONE_FOR[status] ?? "muted";
}

export function StatusBadge({
  status,
  children,
}: Readonly<{ status: string; children?: ReactNode }>) {
  return (
    <span className={`badge badge-${toneFor(status)}`}>
      {children ?? status}
    </span>
  );
}
