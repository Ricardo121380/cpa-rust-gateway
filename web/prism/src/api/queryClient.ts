import { isCancelledError, QueryClient } from "@tanstack/react-query";
import { useSessionStore } from "../session/sessionStore";
import { useVersionStore } from "../features/config-versions/versionStore";
import { asAppError } from "./errors";

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: (count, error) => count < 1 && !isCancelledError(error)
        && asAppError(error).kind === "network" && useSessionStore.getState().unlocked,
      refetchOnWindowFocus: false,
    },
    mutations: { retry: false },
  },
});

// Clear synchronously on either lock or replacement. clear() cancels cached
// queries; the API session guard also rejects late bodies and mutation results.
useSessionStore.subscribe((state, previous) => {
  if (state.generation !== previous.generation) queryClient.clear();
});

// Existing credential/detail queries include identity but not always version
// in their keys. Do not display their previous-version cache on a new draft.
useVersionStore.subscribe((state, previous) => {
  if (state.context?.configVersionId !== previous.context?.configVersionId) {
    queryClient.removeQueries();
  }
});
