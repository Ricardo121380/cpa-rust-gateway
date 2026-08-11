// In-memory session — the ONLY module allowed to hold secret material.
// C6: no browser storage anywhere; refresh clears the session by design.
import { create } from "zustand";

type SessionState = {
  managementKey: string | undefined;
  csrfToken: string | undefined;
  unlocked: boolean;
  unlock: (managementKey: string, csrfToken: string | undefined) => void;
  lock: () => void;
};

export const useSessionStore = create<SessionState>((set) => ({
  managementKey: undefined,
  csrfToken: undefined,
  unlocked: false,
  unlock: (managementKey, csrfToken) =>
    set({ managementKey, csrfToken, unlocked: true }),
  lock: () =>
    set({ managementKey: undefined, csrfToken: undefined, unlocked: false }),
}));

// Closure accessors for the generated client — never export raw values elsewhere.
export const readManagementKey = (): string | undefined =>
  useSessionStore.getState().managementKey;
export const readCsrfToken = (): string | undefined =>
  useSessionStore.getState().csrfToken;

export function isValidManagementKeyShape(value: string): boolean {
  return /^mgmt_[A-Za-z0-9_-]{27,507}$/u.test(value) && value.length >= 32 && value.length <= 512;
}

export function isValidCsrfTokenShape(value: string): boolean {
  return /^csrf_[A-Za-z0-9_-]{27,507}$/u.test(value) && value.length >= 32 && value.length <= 512;
}
