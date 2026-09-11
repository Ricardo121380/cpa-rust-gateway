import { describe, expect, it } from "vitest";
import { NAV_GROUPS, NAV_ITEMS, primaryRoute, workspacePages } from "./navigation";

describe("task-based navigation", () => {
  it("exposes eight primary workspaces and retains all existing destinations", () => {
    const primary = NAV_GROUPS.flatMap((group) => group.items.map((item) => item.to));
    expect(primary).toHaveLength(8);
    expect(primary).not.toContain("/versions");
    expect(primary).not.toContain("/audit");
    expect(NAV_ITEMS).toHaveLength(14);
    for (const item of NAV_ITEMS) expect(primary).toContain(primaryRoute(item.to));
  });
  it("keeps advanced settings and related model/billing pages reachable", () => {
    expect(workspacePages("/audit").map((item) => item.to)).toEqual(["/settings", "/egress", "/runtime", "/versions", "/audit"]);
    expect(workspacePages("/catalog").map((item) => item.to)).toEqual(["/models", "/catalog"]);
    expect(workspacePages("/billing").map((item) => item.to)).toEqual(["/usage", "/billing"]);
    expect(primaryRoute("/")).toBe("/");
  });
});
