import { isCancelledError, QueryClient } from "@tanstack/react-query";
import { useSessionStore } from "../session/sessionStore";
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
