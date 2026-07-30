// English pack. Typed as `Pack` (derived from zh), so a key added to zh.ts
// without a translation here fails the type check instead of silently falling
// back to Chinese.
//
// Not translated on purpose: error codes, scopes, stages, retry decisions,
// endpoint/credential/model identifiers. Those are contract values, not copy —
// translating them would make a log line unsearchable.
import type { Pack } from "./zh";

export const en: Pack = {
  appTitle: "Prism · Gateway admin",
  unlock: {
    title: "Unlock the admin panel",
    managementKey: "Management Key",
    managementKeyHint:
      "mgmt_ prefix, 32-512 alphanumerics or _ -. Paste straight from the server log; newlines and quotes are stripped.",
    csrfToken: "CSRF Token",
    csrfTokenHint: "csrf_ prefix. Required for browser deployments, optional for local CLI access.",
    revealToggle: "Reveal secret",
    submit: "Unlock",
    fillDemo: "Fill demo credentials (fixture mode)",
    hint: "Secrets live in this tab's memory only; a refresh means entering them again.",
    failed: "Admin access unavailable — check the key, your network location, and the deployment config.",
    invalidShape:
      "Wrong shape: a Management Key is mgmt_ followed by 32-512 alphanumerics or _ -.",
  },
  nav: {
    overview: "Overview",
    usage: "Usage",
    monitoring: "Requests",
    versions: "Config versions",
    upstreams: "Upstreams",
    models: "Models & routes",
    access: "Access control",
    egress: "Egress policy",
    runtime: "Runtime",
    audit: "Audit & backup",
    settings: "Settings",
  },
  version: {
    none: "No version selected",
    conflict: "Another session changed this config. The data has been refreshed — review, then retry.",
    conflictAck: "Got it",
    readOnly: "This version is read-only (not a draft).",
    pickerLabel: "Config version",
  },
  state: {
    empty: "No data yet",
    filteredEmpty: "Nothing matches the current filters",
    unavailable: "This deployment does not expose that runtime projection",
    unwired: "Event pipeline not wired yet (G2) — observability appears once the backend lands",
  },
  settings: {
    title: "Settings",
    lead: "Everything here is scoped to this session. The gateway has no settings endpoint and the panel writes no browser storage, so every choice below returns to its default on refresh — deliberately, not for want of implementing it.",

    appearance: "Appearance",
    appearanceHelp: "Follows the system by default. An explicit choice applies to this tab and does not survive a refresh.",
    themeSystem: "Follow system",
    themeLight: "Light",
    themeDark: "Dark",
    themeActive: "In effect",

    language: "Language",
    languageHelp:
      "Also memory-only. UI copy switches immediately; enums and identifiers from the backend are left untranslated because they are part of the contract.",

    session: "Session",
    sessionHelp: "The Management Key and CSRF Token exist in memory only — never on disk, never in a URL.",
    sessionKeyLabel: "Management Key",
    sessionCsrfLabel: "CSRF Token",
    sessionCsrfAbsent: "Not supplied (optional for local CLI access)",
    lock: "Lock and clear secrets",
    lockHelp: "Wipes the in-memory secrets immediately and returns to the unlock screen. Use it before you leave this machine.",

    render: "Rendering",
    renderHelp: "Probed at runtime — these are not switches. They explain why the glass looks the way it does in this browser.",
    lensOn: "True refraction (Chromium)",
    lensOff: "Layered fallback (Firefox / Safari)",
    lensExplain:
      "Firefox and Safari parse backdrop-filter: url() and then render nothing, so a runtime probe decides which path is used.",
    prefReduceMotion: "Reduce motion",
    prefReduceTransparency: "Reduce transparency",
    prefMoreContrast: "Increase contrast",
    prefOn: "On",
    prefOff: "Off",
    prefHelp: "All read from the operating system; the panel only obeys. Enabling them degrades the glass to translucent, then fully solid.",

    build: "Build",
    buildMode: "Mode",
    buildModeDev: "Development (fixture backend available)",
    buildModeProd: "Production",
    buildFixtures: "Fixture backend",
    buildFixturesOn: "Enabled — this data is fabricated locally, not from a real gateway",
    buildFixturesOff: "Disabled — requests go to the real gateway",
    contract: "Contract",
  },
};
