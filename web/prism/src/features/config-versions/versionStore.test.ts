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

it("retains an identified pending batch while viewing active and clears it after publication", () => {
  const state=useVersionStore.getState();
  state.rememberPending(draft);
  state.select(active);
  expect(useVersionStore.getState().pending?.id).toBe(draft.id);
  state.select(draft);
  state.advanceFromEtag("rev-8");
  expect(useVersionStore.getState().pending?.revision).toBe("rev-8");
  state.select({...draft,status:"active",revision:"rev-8"});
  expect(useVersionStore.getState().pending).toBeUndefined();
});
it("does not replace the pending batch with a different draft and clears it on session reset", () => {
  const state=useVersionStore.getState();
  state.rememberPending(draft);
  state.rememberPending({...draft,id:"other"});
  expect(useVersionStore.getState().pending?.id).toBe(draft.id);
  state.reset();
  expect(useVersionStore.getState().pending).toBeUndefined();
});
