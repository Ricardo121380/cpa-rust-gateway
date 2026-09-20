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

type VersionState = {
  context: VersionContext | undefined;
  pending: ConfigVersionSummary | undefined;
  rememberPending: (summary: ConfigVersionSummary) => void;
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
