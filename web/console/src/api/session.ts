// Management credentials live in memory only (never browser storage); a reload requires signing in again.
import { useSyncExternalStore } from "react";

export type Session =
  | { kind: "none" }
  | { kind: "admin"; username: string; token: string; csrf: string; expiresAt: number; mustChangePassword: boolean }
  | { kind: "key"; token: string; csrf: string | null };

/**
 * The configuration graph the console reads and writes. `revision` is the If-Match token of `working`.
 * `loaded` turns true once the version list has been read, so a fresh gateway with no versions at all can be
 * told apart from a context that has not arrived yet.
 */
export type ConfigContext = { active: string | null; working: string | null; workingStatus: "draft" | "active" | "archived" | null; revision: string | null; loaded: boolean };

type State = { session: Session; config: ConfigContext };
const EMPTY_CONFIG: ConfigContext = { active: null, working: null, workingStatus: null, revision: null, loaded: false };
let state: State = { session: { kind: "none" }, config: EMPTY_CONFIG };
const listeners = new Set<() => void>();
const emit = () => listeners.forEach((l) => l());

export const store = {
  get: () => state,
  subscribe: (l: () => void) => { listeners.add(l); return () => listeners.delete(l); },
  setSession(session: Session) { state = { ...state, session }; emit(); },
  setConfig(patch: Partial<ConfigContext>) { state = { ...state, config: { ...state.config, ...patch } }; emit(); },
  signOut() { state = { session: { kind: "none" }, config: EMPTY_CONFIG }; emit(); },
  /** Resolves once the version list has been read for this session; rejects if the session ends first. */
  whenConfigLoaded(): Promise<void> {
    if (state.config.loaded) return Promise.resolve();
    return new Promise((resolve, reject) => {
      const off = store.subscribe(() => {
        if (state.config.loaded) { off(); resolve(); }
        else if (state.session.kind === "none") { off(); reject(new Error("signed out")); }
      });
    });
  },
};

export function useSession() { return useSyncExternalStore(store.subscribe, () => state.session); }
export function useConfigContext() { return useSyncExternalStore(store.subscribe, () => state.config); }

/** Values follow the contract patterns: mgmt_ key (32–512 safe chars), session_/csrf_ + 64 hex. */
export const KEY_PATTERN = /^mgmt_[A-Za-z0-9._~-]{32,512}$/;
export const CSRF_PATTERN = /^csrf_[A-Za-z0-9._~-]{16,512}$/;
