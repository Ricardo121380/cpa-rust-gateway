// Run inside ego-browser nodejs by prepending a configuration object:
// globalThis.PRISM_SESSION_QA = {spaceId, root, origin, mode, output};
// root is an isolated synthetic gateway state containing private qa-password.
// mode is immediate, settled or held. Secrets are never embedded in source.
// The caller owns the existing task space and gateway; this probe closes neither.
const options = globalThis.PRISM_SESSION_QA;
if (!options) throw new Error("Prepend PRISM_SESSION_QA configuration to the EgoLite script");
const fs = await import("node:fs/promises");
const path = await import("node:path");
const origin = new URL(options.origin ?? "");
if (origin.protocol !== "http:" || origin.hostname !== "127.0.0.1" || origin.pathname !== "/"
  || origin.username || origin.password || origin.search || origin.hash) {
  throw new Error("Only a loopback synthetic management listener is allowed");
}
const spaceId = Number(options.spaceId);
if (!Number.isSafeInteger(spaceId) || spaceId <= 0) throw new Error("An active EgoLite task space is required");
const mode = options.mode;
if (!["immediate", "settled", "held"].includes(mode)) throw new Error("Specify immediate, settled, or held timing");
const output = options.output;
if (!output) throw new Error("A receipt path is required");
const root = await fs.realpath(options.root ?? "");
// This explicitly synthetic bootstrap file is used by the local acceptance harness.
// Never point this probe at a production state/credential directory.
const passwordPath = path.join(root, "qa-password");
const stat = await fs.stat(passwordPath);
if (!stat.isFile() || (stat.mode & 0o077) !== 0) throw new Error("QA password must be a private file");
const p = (await taskSpace(spaceId)).page("p1");
await p.goto(`${origin.origin}/admin-ui/#/unlock`);
const { identifier } = await p.cdp("Page.addScriptToEvaluateOnNewDocument", { source: `(() => {
  const events = []; window.__prismQaEvents = events;
  const log = (type, detail) => events.push({t: Math.round(performance.now()), type, detail});
  window.addEventListener('error', e => log('error', e.message));
  window.addEventListener('unhandledrejection', e => log('rejection', String(e.reason?.message ?? e.reason)));
  for (const type of ['hashchange', 'popstate', 'pagehide', 'pageshow', 'DOMContentLoaded'])
    window.addEventListener(type, () => log(type, location.hash));
  const original = window.fetch.bind(window); let grant; let revoked = false; let release;
  window.__prismQaRelease = () => release?.();
  window.fetch = async (...args) => {
    const url = args[0] instanceof Request ? args[0].url : String(args[0]);
    const pathname = new URL(url, location.href).pathname;
    log('fetch', pathname);
    try {
      const response = await original(...args);
      log('response', {path: pathname, status: response.status});
      if (${JSON.stringify(mode)} === 'held' && revoked && response.status === 404 && !release) {
        await new Promise(resolve => { release = resolve; window.__prismQaHeld = true; });
        log('released', pathname);
      }
      if (pathname === '/admin/auth/login' && response.ok) grant = await response.clone().json();
      return response;
    } catch (error) { log('fetch-error', {path: pathname, error: error.name}); throw error; }
  };
  window.__prismQaRevoke = async () => {
    if (!grant) throw Error('No QA login captured');
    const response = await original('/admin/auth/logout', {method:'POST', headers:{
      'X-Management-Key':grant.session_token, 'X-Management-CSRF-Token':grant.csrf_token
    }});
    grant = undefined; revoked = true; log('revoked', response.status); return response.status;
  };
})();` });
try {
  await p.reload();
  await p.waitForSelector('input[name="password"]', { state: "visible" });
  await p.fill('input[name="username"]', "admin");
  await p.fill('input[name="password"]', (await fs.readFile(passwordPath, "utf8")).trim());
  await p.keyboard.press("Escape");
  await p.click('loc=role:heading[name="管理员登录"]');
  await p.click('loc=role:button[name="登录"]');
  await p.waitForSelector("main.canvas", { state: "visible" });
  if (mode === "settled") await p.waitForFunction(() =>
    document.querySelector("main.canvas")?.getAttribute("data-context-version") != null);
  const revocationStatus = await p.evaluate(() => window.__prismQaRevoke());
  if (revocationStatus !== 204) throw new Error(`Unexpected revocation status ${revocationStatus}`);
  await p.goto(`${origin.origin}/admin-ui/#/settings`);
  if (mode === "held") {
    await p.waitForFunction(() => window.__prismQaHeld === true);
    await p.evaluate(() => window.__prismQaRelease());
  }
  let loginVisible = true;
  try { await p.waitForSelector('input[name="password"]', { state: "visible", timeout: 5000 }); }
  catch { loginVisible = false; }
  const observation = await p.evaluate(() => ({
    hash: location.hash, protectedContent: !!document.querySelector("main.canvas"),
    appChildren: document.getElementById("app")?.childElementCount,
    events: window.__prismQaEvents,
  }));
  const errors = observation.events.filter(event => event.type === "error" || event.type === "rejection");
  const passed = loginVisible && observation.hash === "#/unlock" && !observation.protectedContent && errors.length === 0;
  const receipt = { browser: "EgoLite", mode, realGateway: true, heldTransport: mode === "held",
    revocationStatus, loginVisible, passed, ...observation };
  await fs.mkdir(path.dirname(path.resolve(output)), { recursive: true });
  await fs.writeFile(output, JSON.stringify(receipt, null, 2) + "\n");
  console.log({ mode, passed, revocationStatus, loginVisible, errors: errors.length, receipt: path.resolve(output) });
  if (!passed) throw new Error("Session/navigation acceptance failed; inspect the receipt, do not retry blindly");
} finally {
  await p.cdp("Page.removeScriptToEvaluateOnNewDocument", { identifier });
}
