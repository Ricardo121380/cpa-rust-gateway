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

it("adopts a different draft only with the exact prior pending identity and generation",()=>{
 const store=useVersionStore.getState();store.select(active);store.rememberPending(draft);
 const guard={selection:useVersionStore.getState().selectionGeneration,pendingId:draft.id,pendingRevision:draft.revision};
 const other={...draft,id:"chosen-second"};
 expect(store.adoptPending(other,{...guard,selection:guard.selection-1})).toBe(false);
 expect(useVersionStore.getState().pending?.id).toBe(draft.id);
 expect(store.adoptPending(other,guard)).toBe(true);
 expect(useVersionStore.getState().pending?.id).toBe(other.id);
 expect(useVersionStore.getState().context?.configVersionId).toBe(other.id);
 expect(store.adoptPending(draft,guard)).toBe(false);
});
it("a changed pending revision rejects a delayed adoption",()=>{
 const store=useVersionStore.getState();store.select(draft);store.rememberPending(draft);
 const guard={selection:useVersionStore.getState().selectionGeneration,pendingId:draft.id,pendingRevision:draft.revision};
 store.advanceFromEtag("rev-8");expect(store.adoptPending({...draft,id:"other"},guard)).toBe(false);
});
it("observed terminal resolution clears only the exact tracked batch",()=>{
 const store=useVersionStore.getState();store.select(active);store.rememberPending(draft);
 const guard={selection:useVersionStore.getState().selectionGeneration,pendingId:draft.id,pendingRevision:draft.revision};
 expect(store.resolvePending(active,guard)).toBe(false);
 expect(store.resolvePending({...draft,status:"archived"},guard)).toBe(true);
 expect(useVersionStore.getState().pending).toBeUndefined();
 expect(useVersionStore.getState().context?.status).toBe("archived");
});

it("rejects an older observation when explicitly resuming the same pending draft",()=>{
 const store=useVersionStore.getState();store.select(active);store.rememberPending({...draft,revision:"rev-9"});
 const guard={selection:useVersionStore.getState().selectionGeneration,pendingId:draft.id,pendingRevision:"rev-9"};
 expect(store.adoptPending(draft,guard)).toBe(false);
 expect(useVersionStore.getState().pending?.revision).toBe("rev-9");
 expect(useVersionStore.getState().context?.configVersionId).toBe(active.id);
 expect(store.adoptPending({...draft,revision:"rev-10"},guard)).toBe(true);
});
