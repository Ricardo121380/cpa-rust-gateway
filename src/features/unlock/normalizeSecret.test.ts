import { describe, expect, it } from "vitest";
import { normalizeSecret } from "./SecretField";
import { isValidCsrfTokenShape, isValidManagementKeyShape } from "../../session/sessionStore";

const KEY = `mgmt_${"a".repeat(40)}`;
const CSRF = `csrf_${"b".repeat(40)}`;

describe("normalizeSecret", () => {
  it("strips trailing newlines and soft wraps", () => {
    expect(normalizeSecret(`${KEY}\n`)).toBe(KEY);
    expect(normalizeSecret(`mgmt_${"a".repeat(20)}\n  ${"a".repeat(20)}`)).toBe(KEY);
  });

  it("strips wrapping quotes", () => {
    expect(normalizeSecret(`"${KEY}"`)).toBe(KEY);
    expect(normalizeSecret(`'${CSRF}'`)).toBe(CSRF);
  });

  it("strips assignment and Bearer prefixes", () => {
    expect(normalizeSecret(`MGMT_KEY=${KEY}`)).toBe(KEY);
    expect(normalizeSecret(`Bearer:${KEY}`)).toBe(KEY);
    expect(normalizeSecret(`management_key="${KEY}"`)).toBe(KEY);
  });

  it("normalised values pass shape validation", () => {
    expect(isValidManagementKeyShape(normalizeSecret(`  ${KEY}\n`))).toBe(true);
    expect(isValidCsrfTokenShape(normalizeSecret(`"${CSRF}"`))).toBe(true);
  });

  it("leaves a clean value untouched", () => {
    expect(normalizeSecret(KEY)).toBe(KEY);
  });
});
