import { beforeEach, expect, it } from "vitest";
import { useVersionStore, type ConfigVersionSummary } from "./versionStore";

const active: ConfigVersionSummary = { id: "published", revision: "rev-7", status: "active", created_at_ms: 1, description: "" };
const draft: ConfigVersionSummary = { ...active, id: "test-draft", status: "draft" };
beforeEach(() => useVersionStore.getState().reset());

it("defaults to the published configuration even when test drafts arrive first", () => {
  useVersionStore.getState().selectInitialActive([draft, active]);
  expect(useVersionStore.getState().context?.configVersionId).toBe("published");
});
it("never chooses a draft when there is no published configuration", () => {
  useVersionStore.getState().selectInitialActive([draft]);
  expect(useVersionStore.getState().context).toBeUndefined();
});
it("late configuration lists cannot replace an explicitly selected draft or regress its revision", () => {
  useVersionStore.getState().select({ ...draft, revision: "rev-10" });
  const generation = useVersionStore.getState().selectionGeneration;
  useVersionStore.getState().selectInitialActive([active, draft]);
  expect(useVersionStore.getState().context).toEqual({ configVersionId: "test-draft", status: "draft", revision: "rev-10" });
  expect(useVersionStore.getState().selectionGeneration).toBe(generation);
});
