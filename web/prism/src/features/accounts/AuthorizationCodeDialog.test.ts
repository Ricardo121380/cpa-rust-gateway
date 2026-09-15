import { describe, expect, it } from "vitest";
import { existingAuthorizationTarget } from "./AuthorizationCodeDialog";

describe("authorization-code target ownership", () => {
  it("keeps the selected account owner during reauthorization without inventing a binding", () => {
    expect(existingAuthorizationTarget("legacy-codex-owner", "")).toEqual({
      upstream_id:"legacy-codex-owner",
      endpoint_id:"",
    });
    expect(existingAuthorizationTarget(undefined, "endpoint-only")).toBeUndefined();
  });
});
