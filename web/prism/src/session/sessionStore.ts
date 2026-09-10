// In-memory session — the ONLY module allowed to hold secret material.
// C6: no browser storage anywhere; refresh clears the session by design.
import type { AdministratorSession } from "./administrator";
import { create } from "zustand";
import { useVersionStore } from "../features/config-versions/versionStore";

type SessionState = {
  managementKey: string | undefined;
  csrfToken: string | undefined;
  username: string | undefined;
  expiresAt: number | undefined;
  passwordChangeRequired: boolean;
  acceptSession: (session: AdministratorSession) => void;
  unlocked: boolean;
  generation: number;
  unlock: (managementKey: string, csrfToken: string | undefined) => void;
  lock: () => void;
};

export const useSessionStore = create<SessionState>((set) => ({
  managementKey: undefined,
  csrfToken: undefined,
  username: undefined,
  expiresAt: undefined,
  passwordChangeRequired: false,
  unlocked: false,
  generation: 0,
  acceptSession: (session) => {
    const { session_token: managementKey, csrf_token: csrfToken } = session;
    useVersionStore.getState().reset();
    set((state) => ({ managementKey, csrfToken,
      username: session.username, expiresAt: session.expires_at_ms, passwordChangeRequired: session.password_change_required,
      unlocked: true, generation: state.generation + 1 }));
  },
  unlock: (managementKey, csrfToken) => {
    useVersionStore.getState().reset();
    set((state) => ({ managementKey, csrfToken, username: undefined, expiresAt: undefined, passwordChangeRequired: false, unlocked: true, generation: state.generation + 1 }));
  },
  lock: () => {
    useVersionStore.getState().reset();
    set((state) => ({ managementKey: undefined, csrfToken: undefined, username: undefined, expiresAt: undefined, passwordChangeRequired: false, unlocked: false, generation: state.generation + 1 }));
  },
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

// Expiry also locks an idle page, before its next poll or mutation.
let expiryTimer: ReturnType<typeof setTimeout> | undefined;
useSessionStore.subscribe((state, previous) => {
  if (state.generation === previous.generation) return;
  clearTimeout(expiryTimer);
  if (state.expiresAt !== undefined) {
    const generation = state.generation;
    expiryTimer = setTimeout(() => {
      if (useSessionStore.getState().generation === generation) useSessionStore.getState().lock();
    }, Math.max(0, state.expiresAt - Date.now()));
  }
});
