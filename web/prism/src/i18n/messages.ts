// Two packs, one shape. zh-CN is the source of truth: `Pack` is derived from it,
// so adding a key without an English translation is a type error rather than a
// silent fallback to the Chinese text.
//
// The active language lives in memory only — C6 bans browser storage, so it
// resets on refresh, the same contract the session secrets are under. It is
// selectable on the settings page.
import { create } from "zustand";
import { en } from "./en";
import { zh, type Pack } from "./zh";

export type { Pack };
export type Lang = "zh" | "en";

const PACKS: Record<Lang, Pack> = { zh, en };

type LangState = {
  lang: Lang;
  setLang: (lang: Lang) => void;
};

export const useLangStore = create<LangState>((set) => ({
  lang: "zh",
  setLang: (lang) => set({ lang }),
}));

/** Reactive accessor: components re-render when the language changes. */
export function useMessages(): Pack {
  return PACKS[useLangStore((state) => state.lang)];
}

/** The pre-i18n import shape. Kept so pages written against it keep compiling;
 *  it is the zh pack and does NOT react to a language change. New code and any
 *  page being touched should use useMessages() instead. */
export const messages = zh;
