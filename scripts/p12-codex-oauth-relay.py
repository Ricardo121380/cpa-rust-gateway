#!/usr/bin/env python3
"""Forward one OpenAI Codex loopback callback to CPAR's protected management API.

OpenAI registers ``http://localhost:1455/auth/callback`` for the Codex client.  CPAR normally
runs on a remote VPS, so the browser callback has to be forwarded from the operator's Mac.  This
helper deliberately handles exactly one callback, forwards only ``state`` and ``code``, and never
prints the management key, callback URL, authorization code, or response body.

Example (the key is supplied through the environment, never a command-line argument)::

    CPAR_MANAGEMENT_KEY=... python3 scripts/p12-codex-oauth-relay.py \
      --target-url http://127.0.0.1:18193/admin/credentials/credential/oauth/callback \
      --config-version cpar-version

Use an SSH tunnel for a remote management listener, or point ``--target-url`` at the explicitly
admitted management URL.  The relay binds only to loopback and exits after the first callback.
"""

from __future__ import annotations

import argparse
import ipaddress
import json
import os
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.error import HTTPError, URLError
from urllib.parse import parse_qs, urlparse
from urllib.request import HTTPRedirectHandler, Request, build_opener


MAX_CODE_BYTES = 16 * 1024
MAX_STATE_BYTES = 512


class _NoRedirect(HTTPRedirectHandler):
    def redirect_request(self, *_args: object, **_kwargs: object):
        return None


_OPENER = build_opener(_NoRedirect)


def _first(query: dict[str, list[str]], name: str) -> str:
    values = query.get(name, [])
    return values[0] if values else ""


def parse_callback(path: str) -> tuple[str, str, str] | None:
    parsed = urlparse(path)
    if parsed.path != "/auth/callback":
        return None
    query = parse_qs(parsed.query, keep_blank_values=True)
    error = _first(query, "error")
    if error or _first(query, "error_description"):
        state = _first(query, "state")
        if not state or len(state.encode()) > MAX_STATE_BYTES:
            return None
        return state, "", "provider_rejected"
    code = _first(query, "code")
    state = _first(query, "state")
    if not code or not state:
        return None
    if len(code.encode()) > MAX_CODE_BYTES or len(state.encode()) > MAX_STATE_BYTES:
        return None
    return state, code, ""


class CallbackRelay(BaseHTTPRequestHandler):
    server: "RelayServer"

    def do_GET(self) -> None:  # noqa: N802 - BaseHTTPRequestHandler API
        callback = parse_callback(self.path)
        if callback is None:
            self._reply(400, "OAuth callback was not accepted. Restart the OAuth flow.")
            return
        state, code, error = callback
        payload_value = {"state": state}
        if error:
            payload_value["error"] = error
        else:
            payload_value["code"] = code
        payload = json.dumps(payload_value, separators=(",", ":")).encode()
        request = Request(
            self.server.target_url,
            data=payload,
            method="POST",
            headers={
                "Accept": "application/json",
                "Content-Type": "application/json",
                "X-Config-Version": self.server.config_version,
                self.server.key_header: self.server.management_key,
            },
        )
        status = 502
        try:
            with _OPENER.open(request, timeout=self.server.timeout_seconds) as response:
                status = response.status
        except HTTPError as error:
            status = error.code
        except (URLError, TimeoutError, OSError):
            status = 502
        finally:
            # The code and state are released as soon as forwarding is complete; no diagnostic
            # representation is retained by the server object.
            payload = b""
            code = ""
            state = ""
        self.server.forwarded.set()
        if 200 <= status < 300:
            self._reply(200, "CPAR authorization callback received. You may close this tab.")
        else:
            self._reply(502, "CPAR did not accept the callback. Restart the OAuth flow.")

    def log_message(self, *_args: object) -> None:
        return

    def _reply(self, status: int, message: str) -> None:
        body = message.encode()
        self.send_response(status)
        self.send_header("Content-Type", "text/plain; charset=utf-8")
        self.send_header("Cache-Control", "no-store")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)


class RelayServer(ThreadingHTTPServer):
    daemon_threads = True

    def __init__(
        self,
        address: tuple[str, int],
        target_url: str,
        config_version: str,
        management_key: str,
        key_header: str,
        timeout_seconds: float,
    ) -> None:
        super().__init__(address, CallbackRelay)
        self.target_url = target_url
        self.config_version = config_version
        self.management_key = management_key
        self.key_header = key_header
        self.timeout_seconds = timeout_seconds
        self.forwarded = threading.Event()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target-url", required=True)
    parser.add_argument("--config-version", required=True)
    parser.add_argument("--listen-host", default="127.0.0.1")
    parser.add_argument("--listen-port", type=int, default=1455)
    parser.add_argument("--key-header", default="X-Management-Key")
    parser.add_argument("--timeout-seconds", type=float, default=15.0)
    args = parser.parse_args()

    management_key = os.environ.get("CPAR_MANAGEMENT_KEY", "")
    if not management_key or len(management_key.encode()) > 4096:
        parser.error("CPAR_MANAGEMENT_KEY must be set in the environment")
    target = urlparse(args.target_url)
    if (
        target.scheme not in {"http", "https"}
        or not target.netloc
        or target.username is not None
        or target.password is not None
    ):
        parser.error("--target-url must be an absolute HTTP(S) URL")
    if not target.path.endswith("/oauth/callback"):
        parser.error("--target-url must end with /oauth/callback")
    if not 1024 <= args.listen_port <= 65535:
        parser.error("--listen-port must be between 1024 and 65535")
    try:
        listen_address = ipaddress.ip_address(args.listen_host)
    except ValueError:
        parser.error("--listen-host must be a loopback IP address")
    if not listen_address.is_loopback:
        parser.error("--listen-host must be a loopback IP address")
    if args.timeout_seconds <= 0 or args.timeout_seconds > 60:
        parser.error("--timeout-seconds must be between 0 and 60")

    server = RelayServer(
        (str(listen_address), args.listen_port),
        args.target_url,
        args.config_version,
        management_key,
        args.key_header,
        args.timeout_seconds,
    )
    try:
        while not server.forwarded.is_set():
            server.handle_request()
    except KeyboardInterrupt:
        return 130
    finally:
        server.server_close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
