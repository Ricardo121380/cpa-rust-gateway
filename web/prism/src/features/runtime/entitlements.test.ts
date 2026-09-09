import { describe, expect, it } from "vitest";
import { isKnownEntitlement } from "./entitlements";

const evidence = { source: "provider_subscription", confidence: "authoritative", observed_at_ms: 1 };
describe("entitlement evidence keeps domain and source semantics", () => {
  it.each(["grok_build", "chatgpt", "claude"])("recognizes free only in its own domain: %s", (domain) => {
    expect(isKnownEntitlement({ ...evidence, domain, tier: "free" })).toBe(true);
  });
  it("does not apply Build tiers to Web or infer a tier for a new domain", () => {
    expect(isKnownEntitlement({ ...evidence, domain: "grok_web", tier: "supergrok" })).toBe(false);
    expect(isKnownEntitlement({ ...evidence, domain: "grok_console", tier: "free" })).toBe(false);
  });
  it("preserves unknown and refuses to upgrade imported or signed evidence", () => {
    expect(isKnownEntitlement({ ...evidence, domain: "claude", tier: "unknown" })).toBe(true);
    expect(isKnownEntitlement({ ...evidence, domain: "chatgpt", tier: "plus", source: "signed_token" })).toBe(false);
    expect(isKnownEntitlement({ ...evidence, domain: "chatgpt", tier: "plus", source: "signed_token", confidence: "derived" })).toBe(true);
  });
});
