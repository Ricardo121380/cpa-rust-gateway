// Version context — the panel's core mental model (docs/07 §6.1).
// Everything is scoped to one Config Version; only drafts are editable.
import { create } from "zustand";
import { advanceRevision, type VersionContext } from "../../utils/revision";

export type ConfigVersionSummary = Readonly<{
  id: string;
  parent_id?: string | null;
  status: "draft" | "active" | "archived";
  revision: string;
  created_at_ms: number;
  description: string;
}>;

export type PendingSelectionGuard = Readonly<{selection:number;pendingId?:string;pendingRevision?:string}>;

type VersionState = {
  context: VersionContext | undefined;
  pending: ConfigVersionSummary | undefined;
  rememberPending: (summary: ConfigVersionSummary) => void;
  adoptPending: (summary: ConfigVersionSummary, expected: PendingSelectionGuard) => boolean;
  resolvePending: (summary: ConfigVersionSummary, expected: PendingSelectionGuard) => boolean;
  conflict: boolean;
  selectionGeneration: number;
  select: (summary: ConfigVersionSummary) => void;
  selectInitialActive: (versions: readonly ConfigVersionSummary[]) => void;
  advanceFromEtag: (etag: string | null) => void;
  markConflict: () => void;
  clearConflict: () => void;
  reset: () => void;
};

export const useVersionStore = create<VersionState>((set, get) => ({
  context: undefined,
  pending: undefined,
  rememberPending: summary => set(state => ({pending: summary.status === "draft" && (!state.pending || state.pending.id === summary.id) ? {...summary,revision:state.pending?advanceRevision({configVersionId:summary.id,status:"draft",revision:state.pending.revision},summary.revision).revision:summary.revision} : state.pending})),
  adoptPending: (summary,expected) => {
    const state=get();
    if(summary.status!=="draft"||state.selectionGeneration!==expected.selection||state.pending?.id!==expected.pendingId||state.pending?.revision!==expected.pendingRevision)return false;
    const knownRevision=state.pending?.id===summary.id?state.pending.revision:state.context?.configVersionId===summary.id?state.context.revision:undefined;
    if(knownRevision&&advanceRevision({configVersionId:summary.id,status:"draft",revision:knownRevision},summary.revision).revision!==summary.revision)return false;
    set({pending:summary,context:{configVersionId:summary.id,revision:summary.revision,status:summary.status},selectionGeneration:state.selectionGeneration+1,conflict:false});
    return true;
  },
  resolvePending: (summary,expected) => {
    const state=get();
    if(summary.status==="draft"||state.pending?.id!==summary.id||state.selectionGeneration!==expected.selection||state.pending?.id!==expected.pendingId||state.pending?.revision!==expected.pendingRevision)return false;
    set({pending:undefined,context:{configVersionId:summary.id,revision:summary.revision,status:summary.status},selectionGeneration:state.selectionGeneration+1,conflict:false});
    return true;
  },
  conflict: false,
  selectionGeneration: 0,
  selectInitialActive: (versions) => {
    if (get().context !== undefined) return;
    const active = versions.find((version) => version.status === "active");
    if (active !== undefined) get().select(active);
  },
  select: (summary) =>
    set((state) => ({
      pending: state.pending?.id === summary.id ? (summary.status === "draft" ? {...summary,revision:advanceRevision({configVersionId:summary.id,status:"draft",revision:state.pending.revision},summary.revision).revision} : undefined) : state.pending,
      selectionGeneration: state.selectionGeneration + 1,
      context: state.context?.configVersionId === summary.id ? {
        ...advanceRevision(state.context, summary.revision), status: summary.status,
      } : {
        configVersionId: summary.id,
        revision: summary.revision,
        status: summary.status,
      },
      conflict: false,
    })),
  advanceFromEtag: (etag) => {
    const current = get().context;
    if (current === undefined) {
      return;
    }
    const next = advanceRevision(current, etag);
    if (next !== current) {
      set(state=>({ context: next, conflict: false, pending:state.pending?.id===next.configVersionId?{...state.pending,revision:next.revision}:state.pending }));
    }
  },
  markConflict: () => set({ conflict: true }),
  clearConflict: () => set({ conflict: false }),
  reset: () => set((state) => ({ context: undefined, pending: undefined, conflict: false, selectionGeneration: state.selectionGeneration + 1 })),
}));
