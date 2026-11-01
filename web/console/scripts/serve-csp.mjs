// Serve a built bundle with the gateway's exact response headers (CSP included) so CSP violations show
// up locally before embedding. Usage: node scripts/serve-csp.mjs [dir=dist] [port=8790]
import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { extname, join, normalize } from "node:path";

const dir = new URL("../" + (process.argv[2] || "dist") + "/", import.meta.url).pathname;
const port = Number(process.argv[3] || 8790);
const CSP = "default-src 'self'; base-uri 'none'; connect-src 'self'; form-action 'none'; frame-ancestors 'none'; img-src 'self' data:; object-src 'none'; script-src 'self'; style-src 'self'";
const TYPES = { ".html": "text/html; charset=utf-8", ".js": "text/javascript; charset=utf-8", ".css": "text/css; charset=utf-8" };
createServer(async (req, res) => {
  let p = decodeURIComponent(new URL(req.url, "http://x").pathname);
  if (p === "/admin-ui") { res.writeHead(308, { Location: "/admin-ui/" }); return res.end(); }
  p = p.replace(/^\/admin-ui\//, "/");
  if (p === "/") p = "/index.html";
  const file = normalize(join(dir, p));
  if (!file.startsWith(dir)) { res.writeHead(404); return res.end(); }
  try {
    const body = await readFile(file);
    res.writeHead(200, { "Content-Type": TYPES[extname(file)] || "application/octet-stream", "Content-Security-Policy": CSP, "Cache-Control": "no-store", "Referrer-Policy": "no-referrer", "X-Content-Type-Options": "nosniff", "X-Frame-Options": "DENY" });
    res.end(body);
  } catch { res.writeHead(404); res.end(); }
}).listen(port, "127.0.0.1", () => console.log(`serving ${dir} with gateway CSP at http://127.0.0.1:${port}/admin-ui/`));
