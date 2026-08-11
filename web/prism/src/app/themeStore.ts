// Theme preference — memory only (C6 bans browser storage), so it resets on
// refresh, exactly like the session secrets and the language choice.
//
// "system" means: write no attribute at all and let the
// `@media (prefers-color-scheme: dark)` block in tokens.css decide. The explicit
// values write html[data-theme], which is the third layer of the theming chain
// documented at the top of tokens.css.
import { create } from "zustand";

export type ThemeChoice = "system" | "light" | "dark";

type ThemeState = {
  choice: ThemeChoice;
  setChoice: (choice: ThemeChoice) => void;
};

export const useThemeStore = create<ThemeState>((set) => ({
  choice: "system",
  setChoice: (choice) => {
    const root = document.documentElement;
    if (choice === "system") {
      delete root.dataset.theme;
    } else {
      root.dataset.theme = choice;
    }
    set({ choice });
  },
}));

/** What the browser is actually painting right now, which is what the settings
 *  page reports next to the choice — under "system" the choice alone does not
 *  say whether the user is looking at light or dark. */
export function resolvedTheme(choice: ThemeChoice): "light" | "dark" {
  if (choice !== "system") {
    return choice;
  }
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}
