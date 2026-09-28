// Local synthetic API plus the unmodified production bundle and gateway CSP.
// No real gateway, account credentials, provider transport or external egress.
import http from "node:http";
import { readFile, mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve, extname } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { createRequire } from "node:module";

const root = fileURLToPath(new URL("../../../../", import.meta.url));
const require = createRequire(join(root, "web/prism/package.json"));
const { build } = await import(pathToFileURL(require.resolve("esbuild")).href);
const temporary = await mkdtemp(join(tmpdir(), "prism-polish-fixture-"));
const compiled = join(temporary, "fixtures.mjs");
await build({ entryPoints: [join(root, "web/prism/src/dev/fixtures.ts")], outfile: compiled, bundle: true, format: "esm", platform: "node", target: "node22" });
const { fixtureFetch, resetFixturesForTest } = await import(pathToFileURL(compiled).href);
const resources = await readFile(join(root, "crates/gateway-http-actix/src/management_ui_resources.rs"), "utf8");
const csp = resources.match(/const CONTENT_SECURITY_POLICY: &str = "([^"]+)";/)[1];
const assets = resolve(root, "web/prism/dist");

http.createServer(async (request, response) => {
  const url = new URL(request.url, "http://127.0.0.1:5200");
  response.setHeader("Content-Security-Policy", csp);
  response.setHeader("Cache-Control", "no-store");
  try {
    if (url.pathname === "/favicon.ico") { response.writeHead(204); response.end(); return; }
    if (url.pathname === "/__fixture/reset" && request.method === "POST") {
      resetFixturesForTest(); response.writeHead(204); response.end(); return;
    }
    if (url.pathname.startsWith("/admin/")) {
      const chunks = [];
      let length = 0;
      for await (const chunk of request) {
        length += chunk.length;
        if (length > 1048576) throw new Error("fixture request exceeded limit");
        chunks.push(chunk);
      }
      const result = await fixtureFetch(url.href, { method: request.method, headers: request.headers, ...(chunks.length ? { body: Buffer.concat(chunks).toString("utf8") } : {}) });
      response.writeHead(result.status, Object.fromEntries(result.headers));
      response.end(Buffer.from(await result.arrayBuffer()));
      return;
    }
    const relative = url.pathname === "/admin-ui/" ? "index.html" : url.pathname.replace(/^\/admin-ui\//, "");
    const file = resolve(assets, relative);
    if (!file.startsWith(assets + "/")) { response.writeHead(404); response.end(); return; }
    const bytes = await readFile(file);
    response.writeHead(200, { "Content-Type": ({ ".html": "text/html", ".js": "application/javascript", ".css": "text/css" })[extname(file)] });
    response.end(bytes);
  } catch (error) {
    console.error(error.message);
    response.writeHead(500); response.end("local fixture error");
  }
}).listen(5200, "127.0.0.1", () => console.log("Production-bundle synthetic fixture: http://127.0.0.1:5200/admin-ui/"));
