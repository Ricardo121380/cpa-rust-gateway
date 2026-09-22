import { create } from "zustand";
import { useSessionStore } from "../../session/sessionStore";
import { useVersionStore } from "../config-versions/versionStore";

export type CatalogFilters = Readonly<{ provider: string; endpoint: string; credential: string; search: string }>;
export const emptyCatalogFilters: CatalogFilters = { provider: "", endpoint: "", credential: "", search: "" };

// Only read preferences survive page navigation. Never retain selected models,
// directory evidence, mutation receipts or secrets here.
export const useModelWorkspaceFilters = create<{
  catalog: CatalogFilters;
  modelSearch: string;
  modelProvider: string;
  setModelProvider: (value: string) => void;
  setCatalog: (value: CatalogFilters) => void;
  setModelSearch: (value: string) => void;
}>((set) => ({
  catalog: emptyCatalogFilters,
  modelSearch: "",
  modelProvider: "",
  setModelProvider: (modelProvider) => set({ modelProvider }),
  setCatalog: (catalog) => set({ catalog }),
  setModelSearch: (modelSearch) => set({ modelSearch }),
}));

function clearFilters() {
  useModelWorkspaceFilters.setState({ catalog: emptyCatalogFilters, modelSearch: "", modelProvider: "" });
}
useSessionStore.subscribe((state, previous) => {
  if (state.generation !== previous.generation) clearFilters();
});
useVersionStore.subscribe((state, previous) => {
  if (state.selectionGeneration !== previous.selectionGeneration) clearFilters();
});
