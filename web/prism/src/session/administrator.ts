/** Response DTO for the authoritative AdministratorSession schema. Never persisted. */
export type AdministratorSession = Readonly<{
  username: string;
  session_token: string;
  csrf_token: string;
  expires_at_ms: number;
  password_change_required: boolean;
}>;

export function isAdministratorSession(value: unknown): value is AdministratorSession {
  if (typeof value !== "object" || value === null) return false;
  const row = value as Record<string, unknown>;
  return typeof row["username"] === "string" && row["username"].length > 0
    && typeof row["session_token"] === "string" && /^session_[a-f0-9]{64}$/u.test(row["session_token"])
    && typeof row["csrf_token"] === "string" && /^csrf_[a-f0-9]{64}$/u.test(row["csrf_token"])
    && typeof row["expires_at_ms"] === "number" && Number.isSafeInteger(row["expires_at_ms"])
    && row["expires_at_ms"] > Date.now() && row["expires_at_ms"] <= Date.now() + 8 * 60 * 60 * 1000 + 60_000
    && typeof row["password_change_required"] === "boolean";
}
