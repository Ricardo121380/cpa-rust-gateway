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
  conflict: boolean;
  select: (summary: ConfigVersionSummary) => void;
  advanceFromEtag: (etag: string | null) => void;
  markConflict: () => void;
  clearConflict: () => void;
  reset: () => void;
};

export const useVersionStore = create<VersionState>((set, get) => ({
  context: undefined,
  conflict: false,
  select: (summary) =>
    set({
      context: {
        configVersionId: summary.id,
        revision: summary.revision,
        status: summary.status,
      },
      conflict: false,
    }),
  advanceFromEtag: (etag) => {
    const current = get().context;
    if (current === undefined) {
      return;
    }
    const next = advanceRevision(current, etag);
    if (next !== current) {
      set({ context: next, conflict: false });
    }
  },
  markConflict: () => set({ conflict: true }),
  clearConflict: () => set({ conflict: false }),
  reset: () => set({ context: undefined, conflict: false }),
}));
