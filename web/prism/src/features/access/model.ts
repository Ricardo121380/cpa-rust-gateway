// Access-control pure model. Client keys: backend statuses are
// active|disabled|revoked; "expired" is a frontend-derived display state
// (strict now < expires_at_ms — a key is invalid AT its expiry instant).
export type ClientKeyRecord = Readonly<{
  id: string;
  access_group_id: string;
  prefix: string;
  status: "active" | "disabled" | "revoked";
  expires_at_ms?: number | null;
}>;

export type AccessGroupRecord = Readonly<{
  id: string;
  name: string;
  status: "active" | "disabled";
  limits: Readonly<Record<string, number>>;
}>;

export type IssuedClientKey = ClientKeyRecord & Readonly<{ key: string }>;

export type DisplayKeyStatus = "active" | "disabled" | "revoked" | "expired";

export function displayKeyStatus(record: ClientKeyRecord, nowMs: number): DisplayKeyStatus {
  if (record.status !== "active") {
    return record.status;
  }
  if (record.expires_at_ms !== null && record.expires_at_ms !== undefined && nowMs >= record.expires_at_ms) {
    return "expired";
  }
  return "active";
}

export function formatExpiry(expiresAtMs: number | null | undefined): string {
  if (expiresAtMs === null || expiresAtMs === undefined) {
    return "永不过期";
  }
  return new Date(expiresAtMs).toISOString().replace("T", " ").slice(0, 16);
}

export function isValidIdShape(value: string): boolean {
  return value.length >= 1 && value.length <= 128 && value.trim() === value && value.length > 0;
}

// Access-group limits are a free-form object in the contract:
// Record<string, integer >= 0>, at most 16 entries. The table already renders
// them as "key=value key=value", so that same string is what the editor takes
// — display format and input format being one thing means an operator can
// copy a row back into the form without translating it.
const MAX_LIMITS = 16;

export function formatLimits(limits: Readonly<Record<string, number>>): string {
  return Object.entries(limits)
    .map(([key, value]) => `${key}=${value}`)
    .join(" ");
}

export type ParsedLimits =
  | Readonly<{ ok: true; limits: Readonly<Record<string, number>> }>
  | Readonly<{ ok: false; reason: string }>;

export function parseLimits(raw: string): ParsedLimits {
  const trimmed = raw.trim();
  if (trimmed.length === 0) {
    return { ok: true, limits: {} };
  }
  const limits: Record<string, number> = {};
  for (const token of trimmed.split(/\s+/u)) {
    const at = token.indexOf("=");
    if (at <= 0 || at === token.length - 1) {
      return { ok: false, reason: `「${token}」不是 key=value 形式。` };
    }
    const key = token.slice(0, at);
    const rawValue = token.slice(at + 1);
    if (Object.hasOwn(limits, key)) {
      return { ok: false, reason: `限额键「${key}」重复。` };
    }
    if (!/^\d+$/u.test(rawValue)) {
      return { ok: false, reason: `「${key}」的值必须是非负整数,契约不接受 ${rawValue}。` };
    }
    limits[key] = Number(rawValue);
  }
  if (Object.keys(limits).length > MAX_LIMITS) {
    return { ok: false, reason: `限额最多 ${MAX_LIMITS} 项。` };
  }
  return { ok: true, limits };
}
