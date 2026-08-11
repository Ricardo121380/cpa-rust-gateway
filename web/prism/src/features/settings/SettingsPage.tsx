// Settings — deliberately small. The gateway has no settings endpoint and the
// panel writes no browser storage, so this page can only offer things that are
// true for THIS session. It says so rather than presenting session-scoped
// toggles as if they persisted.
//
// Two of its four sections are read-only on purpose: rendering capability and
// OS accessibility preferences are probed, not chosen, and showing them as
// switches would imply the panel can override the operating system.
import { useNavigate } from "react-router-dom";
import { resolvedTheme, useThemeStore, type ThemeChoice } from "../../app/themeStore";
import { useLangStore, useMessages, type Lang } from "../../i18n/messages";
import { useSessionStore } from "../../session/sessionStore";
import { useMediaQuery } from "../../utils/useMediaQuery";
import "./settings.css";

/** Secrets are never rendered, not even masked. A fingerprint is enough to tell
 *  two keys apart, and it cannot be shoulder-surfed or lifted out of the DOM.
 *  The prefix is fixed by the contract so it carries no information; only the
 *  last four characters and the length do. */
function fingerprint(secret: string | undefined): string {
  if (secret === undefined || secret.length < 12) {
    return "—";
  }
  return `…${secret.slice(-4)} · ${secret.length} chars`;
}

export function SettingsPage() {
  const t = useMessages();
  const navigate = useNavigate();

  const choice = useThemeStore((s) => s.choice);
  const setChoice = useThemeStore((s) => s.setChoice);
  const lang = useLangStore((s) => s.lang);
  const setLang = useLangStore((s) => s.setLang);

  const managementKey = useSessionStore((s) => s.managementKey);
  const csrfToken = useSessionStore((s) => s.csrfToken);
  const lock = useSessionStore((s) => s.lock);

  const reduceMotion = useMediaQuery("(prefers-reduced-motion: reduce)");
  const reduceTransparency = useMediaQuery("(prefers-reduced-transparency: reduce)");
  const moreContrast = useMediaQuery("(prefers-contrast: more)");
  const systemDark = useMediaQuery("(prefers-color-scheme: dark)");
  void systemDark; // re-render when the OS flips so `themeActive` stays honest

  const lens = document.documentElement.dataset.lens === "on";

  const THEMES: ReadonlyArray<{ value: ThemeChoice; label: string }> = [
    { value: "system", label: t.settings.themeSystem },
    { value: "light", label: t.settings.themeLight },
    { value: "dark", label: t.settings.themeDark },
  ];
  const LANGS: ReadonlyArray<{ value: Lang; label: string }> = [
    { value: "zh", label: "中文" },
    { value: "en", label: "English" },
  ];

  return (
    <section className="settings-page">
      <header className="page-head">
        <h2>{t.settings.title}</h2>
      </header>

      {/* In a card, not loose on the canvas: out there its backdrop is the
          ambient gradient and it measured 3.5:1 (DESIGN.md §9 rule 3). */}
      <div className="card settings-lead-card">
        <p className="settings-lead">{t.settings.lead}</p>
      </div>

      <div className="card" data-gap="top">
        <div className="card-head">
          <h3>{t.settings.appearance}</h3>
        </div>
        <p className="settings-help">{t.settings.appearanceHelp}</p>
        <div className="settings-choice" role="radiogroup" aria-label={t.settings.appearance}>
          {THEMES.map((option) => (
            <button
              key={option.value}
              type="button"
              role="radio"
              aria-checked={choice === option.value}
              className={choice === option.value ? "chip-on" : "chip-off"}
              onClick={() => setChoice(option.value)}
            >
              {option.label}
            </button>
          ))}
        </div>
        <p className="settings-note">
          {t.settings.themeActive}:{" "}
          <strong>
            {resolvedTheme(choice) === "dark" ? t.settings.themeDark : t.settings.themeLight}
          </strong>
        </p>
      </div>

      <div className="card" data-gap="top">
        <div className="card-head">
          <h3>{t.settings.language}</h3>
        </div>
        <p className="settings-help">{t.settings.languageHelp}</p>
        <div className="settings-choice" role="radiogroup" aria-label={t.settings.language}>
          {LANGS.map((option) => (
            <button
              key={option.value}
              type="button"
              role="radio"
              aria-checked={lang === option.value}
              className={lang === option.value ? "chip-on" : "chip-off"}
              onClick={() => setLang(option.value)}
            >
              {option.label}
            </button>
          ))}
        </div>
      </div>

      <div className="card" data-gap="top">
        <div className="card-head">
          <h3>{t.settings.session}</h3>
        </div>
        <p className="settings-help">{t.settings.sessionHelp}</p>
        <dl className="settings-facts">
          <dt>{t.settings.sessionKeyLabel}</dt>
          <dd className="mono">{fingerprint(managementKey)}</dd>
          <dt>{t.settings.sessionCsrfLabel}</dt>
          <dd className={csrfToken === undefined ? "muted" : "mono"}>
            {csrfToken === undefined ? t.settings.sessionCsrfAbsent : fingerprint(csrfToken)}
          </dd>
        </dl>
        <button
          type="button"
          className="settings-lock"
          onClick={() => {
            lock();
            navigate("/unlock", { replace: true });
          }}
        >
          {t.settings.lock}
        </button>
        <p className="settings-note">{t.settings.lockHelp}</p>
      </div>

      <div className="card" data-gap="top">
        <div className="card-head">
          <h3>{t.settings.render}</h3>
        </div>
        <p className="settings-help">{t.settings.renderHelp}</p>
        <dl className="settings-facts">
          <dt>backdrop-filter</dt>
          <dd>
            <span className={lens ? "badge badge-good" : "badge badge-muted"}>
              {lens ? t.settings.lensOn : t.settings.lensOff}
            </span>
          </dd>
          <dt>{t.settings.prefReduceMotion}</dt>
          <dd>{reduceMotion ? t.settings.prefOn : t.settings.prefOff}</dd>
          <dt>{t.settings.prefReduceTransparency}</dt>
          <dd>{reduceTransparency ? t.settings.prefOn : t.settings.prefOff}</dd>
          <dt>{t.settings.prefMoreContrast}</dt>
          <dd>{moreContrast ? t.settings.prefOn : t.settings.prefOff}</dd>
        </dl>
        <p className="settings-note">{t.settings.lensExplain}</p>
        <p className="settings-note">{t.settings.prefHelp}</p>
      </div>

      <div className="card" data-gap="top">
        <div className="card-head">
          <h3>{t.settings.build}</h3>
        </div>
        <dl className="settings-facts">
          <dt>{t.settings.buildMode}</dt>
          <dd>{import.meta.env.DEV ? t.settings.buildModeDev : t.settings.buildModeProd}</dd>
          <dt>{t.settings.buildFixtures}</dt>
          <dd>
            {import.meta.env.VITE_PRISM_FIXTURES === "1"
              ? t.settings.buildFixturesOn
              : t.settings.buildFixturesOff}
          </dd>
          <dt>{t.settings.contract}</dt>
          <dd className="mono">management-v1</dd>
        </dl>
      </div>
    </section>
  );
}
