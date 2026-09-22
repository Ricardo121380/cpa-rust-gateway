import { describe, expect, it } from "vitest";
import { useSessionStore } from "../../session/sessionStore";
import { useVersionStore } from "../config-versions/versionStore";
import { emptyCatalogFilters, useModelWorkspaceFilters } from "./workspaceFilters";

function seed() {
  useModelWorkspaceFilters.getState().setCatalog({provider:"p",endpoint:"e",credential:"c",search:"Exact/ID"});
  useModelWorkspaceFilters.getState().setModelSearch("another ID");
  useModelWorkspaceFilters.getState().setModelProvider("provider-a");
}
describe("model workspace read preferences", () => {
  it("keeps the two searches separate", () => {
    seed();
    expect(useModelWorkspaceFilters.getState().catalog.search).toBe("Exact/ID");
    expect(useModelWorkspaceFilters.getState().modelSearch).toBe("another ID");
  });
  it("clears on configuration selection and session replacement", () => {
    seed();
    useVersionStore.getState().reset();
    expect(useModelWorkspaceFilters.getState().catalog).toEqual(emptyCatalogFilters);
    expect(useModelWorkspaceFilters.getState().modelSearch).toBe("");
    expect(useModelWorkspaceFilters.getState().modelProvider).toBe("");
    seed();
    useSessionStore.getState().lock();
    expect(useModelWorkspaceFilters.getState().catalog).toEqual(emptyCatalogFilters);
    expect(useModelWorkspaceFilters.getState().modelSearch).toBe("");
    expect(useModelWorkspaceFilters.getState().modelProvider).toBe("");
  });
});
