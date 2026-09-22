import { WorkspaceTabs } from "../../app/WorkspaceTabs";
// Safe service information and session-only display preferences.
import { useEffect, useRef, useState } from "react";
import { Link, useNavigate, useSearchParams } from "react-router-dom";
import { resolvedTheme, useAccessibilityStore, useThemeStore, type ThemeChoice } from "../../app/themeStore";
import { NAV_ITEMS } from "../../app/navigation";
import { useLangStore, useMessages, type Lang } from "../../i18n/messages";
import { logoutAdministrator } from "../../api/client";
import { useSessionStore } from "../../session/sessionStore";
import { useMediaQuery } from "../../utils/useMediaQuery";
import "./settings.css";
import { SystemInformation } from "./SystemInformation";

export function SettingsPage() {
  const t = useMessages();
  const navigate = useNavigate();
  const [params] = useSearchParams();
  const [search, setSearch] = useState("");
  const searchRef = useRef<HTMLInputElement>(null);
  const accessibility = useAccessibilityStore();
  useEffect(() => { if (params.get("focus") === "search") searchRef.current?.focus(); }, [params]);

  const choice = useThemeStore((s) => s.choice);
  const setChoice = useThemeStore((s) => s.setChoice);
  const lang = useLangStore((s) => s.lang);
  const setLang = useLangStore((s) => s.setLang);

  const username = useSessionStore((s) => s.username);
  const expiresAt = useSessionStore((s) => s.expiresAt);

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
        <div><h2>{t.settings.title}</h2><p className="page-description">日常偏好保持轻量，复杂维护留在明确的工作区。</p></div>
      </header>
      <WorkspaceTabs />
      <h3 className="settings-section-title">外观与辅助功能</h3>
      <div className="card settings-preferences">
        <div className="settings-preference-row">
          <div><h3>{t.settings.appearance}</h3><p>当前为{resolvedTheme(choice) === "dark" ? t.settings.themeDark : t.settings.themeLight}；可跟随系统切换。</p></div>
          <div className="settings-choice" role="radiogroup" aria-label={t.settings.appearance}>
            {THEMES.map(option => <button key={option.value} type="button" role="radio" aria-checked={choice === option.value} className={choice === option.value ? "chip-on" : "chip-off"} onClick={() => setChoice(option.value)}>{option.label}</button>)}
          </div>
        </div>
        <div className="settings-preference-row">
          <div><h3>{t.settings.language}</h3>{lang === "en" ? <p>{t.settings.languageCoverage}</p> : <p>工作区界面的显示语言。</p>}</div>
          <div className="settings-choice" role="radiogroup" aria-label={t.settings.language}>
            {LANGS.map(option => <button key={option.value} type="button" role="radio" aria-checked={lang === option.value} className={lang === option.value ? "chip-on" : "chip-off"} onClick={() => setLang(option.value)}>{option.label}</button>)}
          </div>
        </div>
        {([
          ["transparency", "减少透明度", "使用更实的表面，提升内容可读性。"],
          ["contrast", "增强对比度", "加强文字、控件与背景的区分。"],
          ["motion", "减少动态效果", "减少过渡与装饰动画。"],
        ] as const).map(([key, label, description]) => <label className="settings-preference-row" key={key}>
          <span><strong>{label}</strong><span className="settings-preference-description">{description}</span></span>
          <input className="settings-switch" type="checkbox" role="switch" aria-label={label} checked={accessibility[key]} onChange={event => accessibility.set(key, event.target.checked)} />
        </label>)}
        <p className="settings-preferences-note">偏好仅用于当前会话；系统开启的辅助偏好仍然生效。</p>
      </div>


      <div className="card" data-gap="top">
        <div className="card-head">
          <h3>{t.settings.session}</h3>
        </div>
        <p className="settings-help">{t.settings.sessionHelp}</p>
        <dl className="settings-facts">
          <dt>{t.settings.sessionKeyLabel}</dt>
          <dd className="mono">{username ?? "—"}</dd>
          <dt>{t.settings.sessionCsrfLabel}</dt>
          <dd>{expiresAt === undefined ? "—" : new Date(expiresAt).toLocaleString()}</dd>
        </dl>
        <button
          type="button"
          className="settings-lock"
          onClick={() => {
            void logoutAdministrator();
            navigate("/unlock", { replace: true });
          }}
        >
          {t.settings.lock}
        </button>
        <button type="button" className="secondary" onClick={() => navigate("/unlock?change-password=1")}>{t.unlock.changeTitle}</button>
      </div>

      <h3 className="settings-section-title">系统与维护</h3>
      <SystemInformation/>
      <details className="card settings-maintenance" data-gap="top"><summary>高级维护</summary><div className="settings-tool-grid">
        <Link to="/runtime"><strong>运行诊断</strong><span>账号状态、故障与恢复</span></Link>
        <Link to="/egress"><strong>网络与出口</strong><span>访问范围、代理与连接策略</span></Link>
        <Link to="/versions"><strong>配置历史</strong><span>检查待应用修改、差异和回滚</span></Link>
        <Link to="/audit"><strong>操作记录与备份</strong><span>查看变更记录及备份信息</span></Link>
      </div></details>
      <details className="card settings-search" data-gap="top" open={params.get("focus") === "search" ? true : undefined}>
        <summary>{t.navigation.search}</summary>
        <div className="data-toolbar"><input ref={searchRef} aria-label={t.navigation.search} value={search} onChange={(event) => setSearch(event.target.value)} placeholder={t.navigation.search} /></div>
        <div className="section-search">{NAV_ITEMS.filter((item) => t.nav[item.key].toLowerCase().includes(search.toLowerCase())).map((item) => <Link key={item.to} to={item.to}>{t.nav[item.key]}</Link>)}</div>
      </details>
      <details className="card settings-technical" data-gap="top"><summary>{t.settings.render} / {t.settings.build}</summary>
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
      </details>
    </section>
  );
}
