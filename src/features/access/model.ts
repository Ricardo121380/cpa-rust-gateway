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
