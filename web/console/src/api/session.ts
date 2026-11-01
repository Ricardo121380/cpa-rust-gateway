// Management credentials live in memory only (never browser storage); a reload requires signing in again.
import { useSyncExternalStore } from "react";

export type Session =
  | { kind: "none" }
  | { kind: "admin"; username: string; token: string; csrf: string; expiresAt: number; mustChangePassword: boolean }
  | { kind: "key"; token: string; csrf: string | null };

/** The configuration graph the console reads and writes. `revision` is the If-Match token of `working`. */
export type ConfigContext = { active: string | null; working: string | null; workingStatus: "draft" | "active" | "archived" | null; revision: string | null };

type State = { session: Session; config: ConfigContext };
let state: State = { session: { kind: "none" }, config: { active: null, working: null, workingStatus: null, revision: null } };
const listeners = new Set<() => void>();
const emit = () => listeners.forEach((l) => l());

export const store = {
  get: () => state,
  subscribe: (l: () => void) => { listeners.add(l); return () => listeners.delete(l); },
  setSession(session: Session) { state = { ...state, session }; emit(); },
  setConfig(patch: Partial<ConfigContext>) { state = { ...state, config: { ...state.config, ...patch } }; emit(); },
  signOut() { state = { session: { kind: "none" }, config: { active: null, working: null, workingStatus: null, revision: null } }; emit(); },
};

export function useSession() { return useSyncExternalStore(store.subscribe, () => state.session); }
export function useConfigContext() { return useSyncExternalStore(store.subscribe, () => state.config); }

/** Values follow the contract patterns: mgmt_ key (32–512 safe chars), session_/csrf_ + 64 hex. */
export const KEY_PATTERN = /^mgmt_[A-Za-z0-9._~-]{32,512}$/;
export const CSRF_PATTERN = /^csrf_[A-Za-z0-9._~-]{16,512}$/;
